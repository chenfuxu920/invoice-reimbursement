use crate::models::invoice::{HotelDetail, Invoice, InvoiceCategory, InvoiceSource};
use chrono::{NaiveDate, Datelike};
use crate::ocr::structured_output::OcrStructuredOutput;
use crate::ocr::OcrTextItem;
use crate::parser::invoice_type_detector::{InvoiceType, InvoiceTypeDetector};
use regex::Regex;
use uuid::Uuid;

/// 发票区域结构
struct InvoiceRegions {
    header: String,      // 发票号码、开票日期
    buyer: String,       // 购买方信息
    seller: String,      // 销售方信息
    items: String,       // 商品明细（项目名称、金额等）
    total: String,       // 价税合计
    remarks: String,     // 备注
}

/// 从 OcrTextItem 提取 Y 坐标（取顶部），无坐标返回 f64::MAX
fn item_top_y(item: &OcrTextItem) -> f64 {
    item.box_coords.as_ref()
        .and_then(|v| v["points"][0]["y"].as_f64())
        .unwrap_or(f64::MAX)
}
/// 提取 X 坐标（取左侧）
fn item_left_x(item: &OcrTextItem) -> f64 {
    item.box_coords.as_ref()
        .and_then(|v| v["points"][0]["x"].as_f64())
        .unwrap_or(f64::MAX)
}

/// 按坐标排序文本项：先 Y（自上而下）再 X（从左到右），确保阅读顺序
fn sort_texts_by_position(texts: &[OcrTextItem]) -> Vec<&OcrTextItem> {
    let mut items: Vec<&OcrTextItem> = texts.iter().collect();
    items.sort_by(|a, b| {
        let ya = item_top_y(a);
        let yb = item_top_y(b);
        ya.partial_cmp(&yb).unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| item_left_x(a).partial_cmp(&item_left_x(b)).unwrap_or(std::cmp::Ordering::Equal))
    });
    items
}

/// 将发票文本拆分为不同区域
fn split_into_regions(text: &str) -> InvoiceRegions {
    let mut regions = InvoiceRegions {
        header: String::new(),
        buyer: String::new(),
        seller: String::new(),
        items: String::new(),
        total: String::new(),
        remarks: String::new(),
    };

    // 按行处理，识别区域边界
    // 坐标排序确保自上而下阅读顺序：过了价税合计之后不再回退到 seller
    let lines: Vec<&str> = text.lines().collect();
    let mut current_region = "header";
    let mut past_total = false;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 备注区域内永不跳出
        if current_region == "remarks" {
            regions.remarks.push_str(trimmed);
            regions.remarks.push(' ');
            continue;
        }

        // 过了价税合计之后只允许进入备注
        if past_total {
            if trimmed.contains("备注") {
                current_region = "remarks";
            }
            // 其他文本归入当前区域（total 或 remarks）
        } else {
            // 识别区域切换（自上而下顺序，不需防回退）
            if trimmed.contains("购买方") && (trimmed.contains("名称") || trimmed.contains("统一社会信用代码")) {
                current_region = "buyer";
            } else if trimmed.contains("销售方") {
                current_region = "seller";
            } else if trimmed.contains("项目名称") || trimmed.contains("货物或应税劳务") {
                current_region = "items";
            } else if trimmed.contains("价税合计") || trimmed.contains("票价") || trimmed.contains("合计金额") {
                current_region = "total";
                past_total = true;
            } else if trimmed.contains("备注") {
                current_region = "remarks";
            }
        }

        match current_region {
            "header" => { regions.header.push_str(trimmed); regions.header.push(' '); }
            "buyer"  => { regions.buyer.push_str(trimmed);  regions.buyer.push(' '); }
            "seller" => { regions.seller.push_str(trimmed); regions.seller.push(' '); }
            "items"  => { regions.items.push_str(trimmed);  regions.items.push(' '); }
            "total"  => { regions.total.push_str(trimmed);  regions.total.push(' '); }
            "remarks" => { regions.remarks.push_str(trimmed); regions.remarks.push(' '); }
            _ => {}
        }
    }

    regions
}

/// 从 OCR 文本中提取出发/到达城市（仅 Train/Flight 类发票）
fn extract_ticket_cities(text: &str, category: &InvoiceCategory) -> (Option<String>, Option<String>) {
    if *category != InvoiceCategory::Train && *category != InvoiceCategory::Flight {
        return (None, None);
    }

    let mut departure: Option<String> = None;
    let mut arrival: Option<String> = None;

    if *category == InvoiceCategory::Train {
        // 火车票：出发站/发站（带标签）
        let re = Regex::new(r"(?:出发站|发站)[：:]\s*(\S{2,6}(?:站)?)").unwrap();
        departure = re.captures(text).map(|c| station_to_city(c.get(1).unwrap().as_str()));
        let re_arr = Regex::new(r"(?:到达站|到站)[：:]\s*(\S{2,6}(?:站)?)").unwrap();
        arrival = re_arr.captures(text).map(|c| station_to_city(c.get(1).unwrap().as_str()));

        // 火车票兜底：铁路电子客票无标签格式 "G878长沙南站 武汉站"
        if departure.is_none() || arrival.is_none() {
            let re_no_label = Regex::new(
                r"[A-Z]+\d+\s*(\S{2,6}站)\s+(\S{2,6}站)"
            ).unwrap();
            if let Some(caps) = re_no_label.captures(text) {
                if departure.is_none() {
                    departure = Some(station_to_city(caps.get(1).unwrap().as_str()));
                }
                if arrival.is_none() {
                    arrival = Some(station_to_city(caps.get(2).unwrap().as_str()));
                }
            }
        }
    } else {
        // 机票：自/FROM, 至/TO
        let re_dep = Regex::new(r"(?:自|FROM)[：:]\s*(\S{2,10})").unwrap();
        departure = re_dep.captures(text).map(|c| station_to_city(c.get(1).unwrap().as_str()));
        let re_arr = Regex::new(r"(?:至|TO)[：:]\s*(\S{2,10})").unwrap();
        arrival = re_arr.captures(text).map(|c| station_to_city(c.get(1).unwrap().as_str()));
    }

    // 兜底：飞猪等平台票据，备注中城市以 "城市-城市" 格式出现
    // 例如: "2026/05/15 成都-长沙 3U8767 经济舱H"
    if departure.is_none() || arrival.is_none() {
        let re_route = Regex::new(
            r"(\p{Unified_Ideograph}{2,4})[\s]*[-－—][\s]*(\p{Unified_Ideograph}{2,4})"
        ).unwrap();
        if let Some(caps) = re_route.captures(text) {
            let raw_dep = caps.get(1).unwrap().as_str().trim();
            let raw_arr = caps.get(2).unwrap().as_str().trim();
            if departure.is_none() {
                departure = Some(station_to_city(raw_dep));
            }
            if arrival.is_none() {
                arrival = Some(station_to_city(raw_arr));
            }
        }
    }

    (departure, arrival)
}

/// 从票据 OCR 文本中提取票面实际出行日期（非开票日期）
fn extract_ticket_travel_date(text: &str, category: &InvoiceCategory) -> Option<NaiveDate> {
    if *category != InvoiceCategory::Train && *category != InvoiceCategory::Flight {
        return None;
    }

    // 格式1: "2026/05/15" — 飞猪等平台备注中的日期
    let re_slash = Regex::new(r"(\d{4})/(\d{1,2})/(\d{1,2})").unwrap();
    if let Some(caps) = re_slash.captures(text) {
        let y: i32 = caps.get(1)?.as_str().parse().ok()?;
        let m: u32 = caps.get(2)?.as_str().parse().ok()?;
        let d: u32 = caps.get(3)?.as_str().parse().ok()?;
        if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
            if date.year() >= 2020 && date.year() <= 2100 {
                return Some(date);
            }
        }
    }

    // 格式2: "2025年11月14日 15:22开" — 铁路电子客票（后跟发车时间，区别于开票日期）
    let re_cn = Regex::new(r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日\s+\d{1,2}:\d{2}").unwrap();
    if let Some(caps) = re_cn.captures(text) {
        let y: i32 = caps.get(1)?.as_str().parse().ok()?;
        let m: u32 = caps.get(2)?.as_str().parse().ok()?;
        let d: u32 = caps.get(3)?.as_str().parse().ok()?;
        if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
            if date.year() >= 2020 && date.year() <= 2100 {
                return Some(date);
            }
        }
    }

    // 格式3: "2025-11-14" — ISO 日期
    let re_iso = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap();
    if let Some(caps) = re_iso.captures(text) {
        let y: i32 = caps.get(1)?.as_str().parse().ok()?;
        let m: u32 = caps.get(2)?.as_str().parse().ok()?;
        let d: u32 = caps.get(3)?.as_str().parse().ok()?;
        if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
            if date.year() >= 2020 && date.year() <= 2100 {
                return Some(date);
            }
        }
    }

    None
}

/// 站名/机场名归一化为城市名
fn station_to_city(raw: &str) -> String {
    let mut s = raw.trim().to_string();

    // 去除常见后缀（按序处理，长的先匹配）
    for suffix in &["国际机场", "机场", "东站", "西站", "南站", "北站", "站"] {
        if s.ends_with(suffix) {
            s = s[..s.len() - suffix.len()].to_string();
            break;
        }
    }

    // 去除机场三字码（如 PEK / SHA）
    let re_code = Regex::new(r"\s*[A-Z]{3}$").unwrap();
    s = re_code.replace(&s, "").to_string();

    // 兜底映射表（已知片区/镇/区 → 城市）
    let mapping: std::collections::HashMap<&str, &str> = [
        ("虹桥", "上海"), ("宝安", "深圳"), ("江北", "重庆"),
        ("流亭", "青岛"), ("龙嘉", "长春"), ("太平", "哈尔滨"),
        ("遥墙", "济南"), ("周水子", "大连"), ("双流", "成都"),
        ("天河", "武汉"), ("黄花", "长沙"), ("咸阳", "西安"),
        ("滨海", "天津"), ("长水", "昆明"), ("萧山", "杭州"),
    ].iter().cloned().collect();

    // 直接映射匹配
    if let Some(city) = mapping.get(s.as_str()) {
        return city.to_string();
    }

    // 检查是否以映射表 key 结尾（如 "上海虹桥" → "虹桥" → "上海"）
    for (key, city) in &mapping {
        if s.ends_with(key) {
            return city.to_string();
        }
    }

    // 已知主要城市前缀（2字）
    let major_cities = ["北京", "上海", "广州", "深圳", "成都", "杭州", "南京",
                        "武汉", "天津", "重庆", "西安", "长沙", "昆明", "青岛",
                        "大连", "厦门", "哈尔滨", "长春", "济南", "沈阳"];

    // 去除方向后缀后检查是否为已知城市（如 "北京南" → "北京"）
    for dir in &["东", "南", "西", "北"] {
        if s.ends_with(dir) && s.len() > dir.len() {
            let candidate = &s[..s.len() - dir.len()];
            if major_cities.contains(&candidate) {
                return candidate.to_string();
            }
        }
    }

    // 检查是否以已知城市开头 + 剩余部分（如 "成都双流" → "成都" + "双流"、"北京首都" → "北京" + "首都"）
    for city in &major_cities {
        if s.starts_with(city) && s.len() > city.len() {
            let rest = &s[city.len()..];
            if mapping.contains_key(rest) || ["东", "南", "西", "北"].contains(&rest) || rest.len() >= 2 {
                return city.to_string();
            }
        }
    }

    // 如果已经是纯城市名（2-4 字），直接返回
    if s.chars().count() >= 2 && s.chars().count() <= 4 {
        return s;
    }

    raw.trim().to_string()
}

pub fn parse_invoice_text(
    texts: &[OcrTextItem],
    source: InvoiceSource,
) -> Result<Invoice, String> {
    // 按坐标重排：先 Y（自上而下）再 X（从左到右）
    let sorted_texts = sort_texts_by_position(texts);

    let all_text: String = sorted_texts
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let regions = split_into_regions(&all_text);

    eprintln!("[hotel_debug] === parse_invoice_text ===");
    eprintln!("[hotel_debug] all_text (first 500 chars): {}", &all_text.chars().take(500).collect::<String>());
    eprintln!("[hotel_debug] regions.header.len={}, .buyer.len={}, .seller.len={}, .items.len={}, .total.len={}, .remarks.len={}",
        regions.header.len(), regions.buyer.len(), regions.seller.len(), regions.items.len(), regions.total.len(), regions.remarks.len());
    eprintln!("[hotel_debug] regions.items: {}", &regions.items.chars().take(300).collect::<String>());
    eprintln!("[hotel_debug] regions.remarks: {}", &regions.remarks.chars().take(500).collect::<String>());
    eprintln!("[hotel_debug] regions.seller: {}", &regions.seller.chars().take(200).collect::<String>());

    let amount = match extract_amount(&regions.total) {
        Ok(amt) => amt,
        Err(_) => extract_amount(&all_text)?,
    };
    let mut seller_name = extract_seller_name(&regions.seller);
    if seller_name.is_empty() {
        seller_name = extract_seller_by_coords(texts);
    }
    if seller_name.is_empty() {
        seller_name = extract_seller_name(&all_text);
    }
    // ponytail: parangi 列交错回退。双栏发票被 parangi 按字符交错合并时
    // （"销购"="销售"+"购买"），"名称:"后的文本是乱码，但销售方公司名
    // 完整出现在"名称:"标签之前。用公司名后缀模式从全文恢复。
    // PyMuPDF 场景：标签(KaiTi)和值(SimSun)是不同 span，正则可能抓到
    // 税号而非公司名——含10+位数字或无中文字符的销售方一定是乱码。
    if seller_name.is_empty()
        || seller_name.contains("名称")
        || seller_name.contains("售买")
        || seller_name.contains('<')
        || seller_name.contains('>')
        || !seller_name.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
        || seller_name.chars().filter(|c| c.is_ascii_digit()).count() >= 10
    {
        if let Some(name) = extract_company_name_fallback(&all_text) {
            seller_name = name;
        }
    }
    let item_name = extract_item_name(&regions.items);
    let date = extract_date(&all_text);
    let invoice_number = extract_invoice_number(&regions.header);

    let mut category = classify_from_regions(&regions.items, &regions.seller, &item_name, &seller_name);
    eprintln!("[hotel_debug] classify_from_regions → {:?} (items_has_住宿服务={}, items_has_生产生活={})",
        category, regions.items.contains("*住宿服务*"), regions.items.contains("*生产生活服务*住宿费"));

    if category == InvoiceCategory::Other {
        let blocks: Vec<_> = texts.iter().map(|t| {
            crate::ocr::structured_output::OcrTextBlock {
                text: t.text.clone(),
                confidence: t.confidence,
                bbox: crate::ocr::structured_output::BoundingBox::default(),
                line_index: 0,
                block_type: crate::ocr::structured_output::TextBlockType::Other,
            }
        }).collect();
        let ocr_output = crate::ocr::structured_output::OcrStructuredOutput {
            blocks,
            layout: crate::ocr::structured_output::PageLayout::default(),
        };
        let invoice_type = InvoiceTypeDetector::detect(&ocr_output);
        eprintln!("[hotel_debug] classify_from_regions returned Other, invoice_type={:?}", invoice_type);
        category = classify_from_full_text(&ocr_output, &invoice_type);
        eprintln!("[hotel_debug] classify_from_full_text → {:?}", category);
    }

    eprintln!("[hotel_debug] final category={:?}, amount={}, seller={}, date={:?}", category, amount, seller_name, date);

    // pdfplumber 多栏合并可能把"备"和"注"拆散，"备注"区域检测为空时回退到 seller 区域
    let effective_remarks = if category == InvoiceCategory::Hotel && regions.remarks.is_empty()
        && !regions.seller.is_empty()
    {
        eprintln!("[hotel_debug] remarks empty, using seller as fallback (pdfplumber split '备注' into '备'+'注')");
        &regions.seller
    } else {
        &regions.remarks
    };

    let hotel_detail = if category == InvoiceCategory::Hotel {
        let remarks_nights = parse_nights_from_remarks(effective_remarks);
        let item_quantity = extract_item_quantity(&regions.items);
        let detail = parse_hotel_detail(effective_remarks, date);
        let statement = extract_hotel_statement_detail(&all_text, date);
        eprintln!("[hotel_debug] remarks_nights={:?}, item_quantity={:?}, detail={:?}, statement={:?}",
            remarks_nights, item_quantity,
            detail.as_ref().map(|d| format!("{}-{} nights={}", d.check_in.map_or("?".into(), |c| c.to_string()), d.check_out.map_or("?".into(), |c| c.to_string()), d.nights)),
            statement.as_ref().map(|s| format!("{}-{} nights={}", s.check_in.map_or("?".into(), |c| c.to_string()), s.check_out.map_or("?".into(), |c| c.to_string()), s.nights)));
        // 交叉验证：备注天数 vs 商品数量 vs 结账单入住/离店日期
        // 商品数量列中 q=1 是行项目数不是住宿天数，不可靠；q>1 才可信
        let nights = match (remarks_nights, item_quantity) {
            (Some(r), Some(q)) if r != q => r.max(q),
            (Some(r), _) => r,
            (_, Some(q)) if q > 1 => q,
            _ => detail.as_ref().map(|d| d.nights)
                .or_else(|| statement.as_ref().map(|s| s.nights))
                .unwrap_or(1),
        };
        eprintln!("[hotel_debug] final nights={}", nights);
        let base_detail = detail.or(statement).unwrap_or(HotelDetail {
            check_in: None,
            check_out: None,
            nights,
            nightly_rate: amount / nights.max(1) as f64,
        });
        Some(HotelDetail {
            nights,
            ..base_detail
        })
    } else {
        None
    };

    // 提取票据出发/到达城市（仅 Train/Flight 类发票）
    let (departure_city, arrival_city) = extract_ticket_cities(&all_text, &category);
    let travel_date = extract_ticket_travel_date(&all_text, &category);

    // 通行费发票"备注"二字常为竖排印刷，OCR 识别不到导致 remarks 区域为空。
    // 此时通过坐标从价税合计下方恢复备注文本。
    let effective_remarks = if category == InvoiceCategory::Toll && regions.remarks.is_empty() {
        extract_toll_remarks_by_coords(texts)
    } else {
        regions.remarks.clone()
    };

    let toll_travel_time = if category == InvoiceCategory::Toll {
        extract_toll_travel_time(&effective_remarks)
    } else {
        None
    };

    Ok(Invoice {
        id: Uuid::new_v4().to_string(),
        invoice_number,
        amount,
        seller_name,
        item_name,
        date,
        travel_date,
        category,
        source,
        itineraries: vec![],
        itinerary_file: None,
        remarks: effective_remarks,
        hotel_detail,
        departure_city,
        arrival_city,
        toll_travel_time,
    })
}

/// 从备注栏解析住宿发票详情
/// 直接匹配日期范围模式 "M-DD至M-DD"，不依赖前缀标签
/// 支持: "订单日期:4-24至4-27"、"入离日期:5-25至5-29"、"入住时间:6-1至6-3" 等
fn parse_hotel_detail(remarks: &str, invoice_date: NaiveDate) -> Option<HotelDetail> {
    // 在备注中搜索第一个 "M-DD至M-DD" 模式（不要求特定前缀）
    let date_re = Regex::new(r"(\d{1,2})-(\d{1,2})\s*至\s*(\d{1,2})-(\d{1,2})").ok()?;
    let caps = date_re.captures(remarks)?;
    let in_month: u32 = caps.get(1)?.as_str().parse().ok()?;
    let in_day: u32 = caps.get(2)?.as_str().parse().ok()?;
    let out_month: u32 = caps.get(3)?.as_str().parse().ok()?;
    let out_day: u32 = caps.get(4)?.as_str().parse().ok()?;

    let in_year = resolve_year_for_stay(in_month, invoice_date);
    let out_year = resolve_year_for_stay(out_month, invoice_date);
    let check_in = NaiveDate::from_ymd_opt(in_year, in_month, in_day)?;
    let check_out = NaiveDate::from_ymd_opt(out_year, out_month, out_day)?;
    let nights = (check_out - check_in).num_days().max(1) as usize;

    Some(HotelDetail {
        check_in: Some(check_in),
        check_out: Some(check_out),
        nights,
        nightly_rate: 0.0, // 后续由 form_builder 计算
    })
}

/// 根据入住月份推断年份（处理跨年场景）
/// 发票日期上半年 + 入住月份下半年 → 前一年
/// 发票日期下半年 + 入住月份上半年 → 后一年
/// 同半边 → 发票年份
fn resolve_year_for_stay(stay_month: u32, invoice_date: NaiveDate) -> i32 {
    let inv_year = invoice_date.year();
    let inv_month = invoice_date.month();
    if stay_month > 6 && inv_month <= 6 {
        inv_year - 1  // e.g. 发票1月，入住12月 → 去年
    } else if stay_month <= 6 && inv_month > 6 {
        inv_year + 1  // e.g. 发票12月，入住1月 → 明年
    } else {
        inv_year       // 同半边
    }
}

/// 从备注栏解析 "共N天" 获取天数
/// 支持 "共3天"、"共 3 天"（OCR分行后含空格）
fn parse_nights_from_remarks(remarks: &str) -> Option<usize> {
    let re = Regex::new(r"共\s*(\d+)\s*天").ok()?;
    let caps = re.captures(remarks)?;
    caps.get(1)?.as_str().parse().ok()
}

/// 从商品明细区域提取数量（住宿发票明细行中的数量列）
/// 格式: "*住宿服务*住宿费  1  420.00" 或 "*生产生活服务*住宿费 天 7 340.70"
/// pdfplumber 多栏可能合并数字: "天 7340.70" (7+340.70)
fn extract_item_quantity(items_text: &str) -> Option<usize> {
    // 标准格式：数量后有空格分隔
    let re = Regex::new(r"\*(?:住宿服务|生产生活服务)\*.*?\s+(?:天\s+)?(\d+)\s+[\d,.]+").ok()?;
    if let Some(caps) = re.captures(items_text) {
        return caps.get(1)?.as_str().parse().ok();
    }
    // pdfplumber 合并数字兜底：天 后取第1个数字（"天 7340.70"→7）
    let re_merged = Regex::new(r"天\s*(\d)").ok()?;
    let caps = re_merged.captures(items_text)?;
    caps.get(1)?.as_str().parse().ok()
}

/// 从酒店结账单全文提取入住/离店日期和住宿天数
/// 支持多种日期格式：2026-04-27、2026年04月27日、2026/04/27、2026.04.27
/// 无年份时（04-27、4月27日）使用发票日期年份补全
/// 标签可能带有可选的中文/英文冒号
fn extract_hotel_statement_detail(text: &str, invoice_date: NaiveDate) -> Option<HotelDetail> {
    let check_in = extract_labeled_date(text, "入住日期", invoice_date)?;
    let check_out = extract_labeled_date(text, "离店日期", invoice_date)?;
    let nights = (check_out - check_in).num_days().max(1) as usize;

    Some(HotelDetail {
        check_in: Some(check_in),
        check_out: Some(check_out),
        nights,
        nightly_rate: 0.0,
    })
}

/// 从文本中提取标签后的日期，支持多种日期格式
/// 无年份时使用 invoice_date + H1/H2 跨年推断
fn extract_labeled_date(text: &str, label: &str, invoice_date: NaiveDate) -> Option<NaiveDate> {
    let label_escaped = regex::escape(label);
    let pattern = format!(r"{}[:：]?\s*([^\n]+)", label_escaped);
    let label_re = Regex::new(&pattern).ok()?;
    let date_str = label_re.captures(text)?.get(1)?.as_str().trim();
    parse_date_flexible(date_str, invoice_date)
}

/// 按优先级尝试多种日期格式解析
fn parse_date_flexible(s: &str, invoice_date: NaiveDate) -> Option<NaiveDate> {
    // 1. 中文格式：YYYY年M月D日
    let re = Regex::new(r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日").unwrap();
    if let Some(caps) = re.captures(s) {
        let y: i32 = caps[1].parse().ok()?;
        let m: u32 = caps[2].parse().ok()?;
        let d: u32 = caps[3].parse().ok()?;
        return NaiveDate::from_ymd_opt(y, m, d);
    }
    // 2. ISO 横线：YYYY-MM-DD
    let re = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap();
    if let Some(caps) = re.captures(s) {
        let y: i32 = caps[1].parse().ok()?;
        let m: u32 = caps[2].parse().ok()?;
        let d: u32 = caps[3].parse().ok()?;
        return NaiveDate::from_ymd_opt(y, m, d);
    }
    // 3. 斜线：YYYY/MM/DD
    let re = Regex::new(r"(\d{4})/(\d{2})/(\d{2})").unwrap();
    if let Some(caps) = re.captures(s) {
        let y: i32 = caps[1].parse().ok()?;
        let m: u32 = caps[2].parse().ok()?;
        let d: u32 = caps[3].parse().ok()?;
        return NaiveDate::from_ymd_opt(y, m, d);
    }
    // 4. 点：YYYY.MM.DD
    let re = Regex::new(r"(\d{4})\.(\d{2})\.(\d{2})").unwrap();
    if let Some(caps) = re.captures(s) {
        let y: i32 = caps[1].parse().ok()?;
        let m: u32 = caps[2].parse().ok()?;
        let d: u32 = caps[3].parse().ok()?;
        return NaiveDate::from_ymd_opt(y, m, d);
    }
    // === 无年份格式，通过 H1/H2 推断跨年 ===
    // 5. 横线无年份：MM-DD（也匹配 M-DD 或 M-D）
    let re = Regex::new(r"(\d{1,2})-(\d{1,2})$").unwrap();
    if let Some(caps) = re.captures(s) {
        let m: u32 = caps[1].parse().ok()?;
        let d: u32 = caps[2].parse().ok()?;
        let y = resolve_year_for_stay(m, invoice_date);
        return NaiveDate::from_ymd_opt(y, m, d);
    }
    // 6. 斜线无年份：MM/DD
    let re = Regex::new(r"(\d{1,2})/(\d{1,2})$").unwrap();
    if let Some(caps) = re.captures(s) {
        let m: u32 = caps[1].parse().ok()?;
        let d: u32 = caps[2].parse().ok()?;
        let y = resolve_year_for_stay(m, invoice_date);
        return NaiveDate::from_ymd_opt(y, m, d);
    }
    // 7. 点无年份：MM.DD
    let re = Regex::new(r"(\d{1,2})\.(\d{1,2})$").unwrap();
    if let Some(caps) = re.captures(s) {
        let m: u32 = caps[1].parse().ok()?;
        let d: u32 = caps[2].parse().ok()?;
        let y = resolve_year_for_stay(m, invoice_date);
        return NaiveDate::from_ymd_opt(y, m, d);
    }
    // 8. 中文无年份：M月D日
    let re = Regex::new(r"(\d{1,2})\s*月\s*(\d{1,2})\s*日$").unwrap();
    if let Some(caps) = re.captures(s) {
        let m: u32 = caps[1].parse().ok()?;
        let d: u32 = caps[2].parse().ok()?;
        let y = resolve_year_for_stay(m, invoice_date);
        return NaiveDate::from_ymd_opt(y, m, d);
    }
    None
}

/// 基于区域的分类（更准确）
fn classify_from_regions(
    items_text: &str,
    seller_text: &str,
    item_name: &str,
    seller_name: &str,
) -> InvoiceCategory {
    // 1. 优先匹配商品明细中的服务类型码（最可靠）
    if items_text.contains("*住宿服务*") || items_text.contains("*生产生活服务*住宿费") {
        return InvoiceCategory::Hotel;
    }
    // *运输服务*/客运服务 可被火车票/机票共用，需先排除后再归为市内交通
    if items_text.contains("*运输服务*") || items_text.contains("*客运服务*") {
        let items_lower = items_text.to_lowercase();
        if contains_any(&items_lower, &["火车", "高铁", "铁路"]) {
            return InvoiceCategory::Train;
        }
        if contains_any(&items_lower, &["航空", "机票"]) {
            return InvoiceCategory::Flight;
        }
        return InvoiceCategory::CityTransport;
    }
    if items_text.contains("*航空运输服务*") || items_text.contains("*旅客运输服务*") {
        return InvoiceCategory::Flight;
    }
    if items_text.contains("*餐饮服务*") {
        return InvoiceCategory::Meal;
    }

    // 2. 匹配商品名称关键词
    let item_lower = item_name.to_lowercase();
    if contains_any(&item_lower, &["住宿", "酒店", "宾馆", "民宿"]) {
        return InvoiceCategory::Hotel;
    }
    // 保险费发票优先识别（防止"机票航空意外险"被误判为机票）
    if contains_any(&item_lower, &["保险", "意外险"]) {
        return InvoiceCategory::Insurance;
    }
    if contains_any(&item_lower, &["机票", "航空", "航班"]) {
        return InvoiceCategory::Flight;
    }
    if contains_any(&item_lower, &["火车", "高铁", "铁路"]) {
        return InvoiceCategory::Train;
    }
    if contains_any(&item_lower, &["餐饮", "饭店", "餐厅"]) {
        return InvoiceCategory::Meal;
    }

    // 2.5 检查商品明细区域全文（item_name 提取可能失败，退化检查 items_text）
    let items_lower = items_text.to_lowercase();
    // 保险费发票优先识别（防止"机票航空意外险"被误判为机票）
    // 真实样本：items_text 含"*保险服务*国内机票航空意外"，"机票"会误命中 Flight
    if contains_any(&items_lower, &["保险服务", "意外险", "保险费"]) {
        return InvoiceCategory::Insurance;
    }
    if contains_any(&items_lower, &["机票", "航空", "航班", "旅客运输"]) {
        return InvoiceCategory::Flight;
    }
    if contains_any(&items_lower, &["退票", "改签", "手续费"]) {
        return InvoiceCategory::TicketChange;
    }
    if contains_any(&items_lower, &["火车", "高铁", "铁路"]) {
        return InvoiceCategory::Train;
    }
    if contains_any(&items_lower, &["住宿", "酒店", "宾馆"]) {
        return InvoiceCategory::Hotel;
    }

    // 3. 匹配销售方名称关键词
    let seller_lower = seller_name.to_lowercase();
    if contains_any(&seller_lower, &["滴滴", "高德", "网约车", "t3", "曹操"]) {
        return InvoiceCategory::CityTransport;
    }
    if contains_any(&seller_lower, &["酒店", "宾馆", "住宿"]) {
        return InvoiceCategory::Hotel;
    }
    if contains_any(&seller_lower, &["航空", "机票"]) {
        return InvoiceCategory::Flight;
    }
    if contains_any(&seller_lower, &["铁路", "高铁", "火车"]) {
        return InvoiceCategory::Train;
    }

    // 3.5 检查销售方区域全文
    let seller_full_lower = seller_text.to_lowercase();
    if contains_any(&seller_full_lower, &["航空", "机票"]) {
        return InvoiceCategory::Flight;
    }
    if contains_any(&seller_full_lower, &["铁路", "高铁", "火车"]) {
        return InvoiceCategory::Train;
    }

    InvoiceCategory::Other
}

pub fn classify_from_full_text(
    ocr: &OcrStructuredOutput,
    invoice_type: &InvoiceType,
) -> InvoiceCategory {
    let all_text = ocr
        .blocks
        .iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    if all_text.contains("*住宿服务*") || all_text.contains("*生产生活服务*住宿费") {
        return InvoiceCategory::Hotel;
    }
    // *运输服务*/客运服务 可被火车票/机票共用，先排除后再归为市内交通
    if all_text.contains("*运输服务*") || all_text.contains("*客运服务*") {
        if contains_any(&all_text, &["火车", "高铁", "铁路"]) {
            return InvoiceCategory::Train;
        }
        if contains_any(&all_text, &["航空", "机票"]) {
            return InvoiceCategory::Flight;
        }
        return InvoiceCategory::CityTransport;
    }
    if all_text.contains("*航空运输服务*") || all_text.contains("*旅客运输服务*") {
        return InvoiceCategory::Flight;
    }

    // 保险费发票优先识别（防止"机票航空意外险"被误判为机票）
    // 必须在 InvoiceType match 之前，避免 FlightInvoice 短路
    if contains_any(&all_text, &["保险服务", "意外险", "保险费"]) {
        return InvoiceCategory::Insurance;
    }

    match invoice_type {
        InvoiceType::FlightInvoice => return InvoiceCategory::Flight,
        InvoiceType::TrainInvoice => return InvoiceCategory::Train,
        InvoiceType::HotelStatement => return InvoiceCategory::Hotel,
        InvoiceType::RideHailingInvoice | InvoiceType::RideHailingItinerary => {
            return InvoiceCategory::CityTransport
        }
        InvoiceType::TollInvoice => return InvoiceCategory::Toll,
        _ => {}
    }

    // 通行费兜底：InvoiceTypeDetector 未命中时，通过关键词识别高速通行费
    if contains_any(&all_text, &["通行费", "过路费", "etc"]) {
        return InvoiceCategory::Toll;
    }

    if contains_any(&all_text, &["酒店", "宾馆", "住宿", "招待所", "民宿"]) {
        return InvoiceCategory::Hotel;
    }
    if contains_any(&all_text, &["滴滴", "网约车", "高德", "t3", "曹操", "出租"]) {
        return InvoiceCategory::CityTransport;
    }
    // 保险/退改签优先于航班检查（防止"机票航空意外险"误判为机票）
    if contains_any(&all_text, &["保险"]) {
        return InvoiceCategory::Insurance;
    }
    if contains_any(&all_text, &["退票", "改签"]) {
        return InvoiceCategory::TicketChange;
    }
    if contains_any(&all_text, &["航空", "机票", "机场", "航班"]) {
        return InvoiceCategory::Flight;
    }
    if contains_any(&all_text, &["铁路", "高铁", "火车", "客运站"]) {
        return InvoiceCategory::Train;
    }
    if contains_any(&all_text, &["餐饮", "饭店", "食品", "餐厅", "饭馆"]) {
        return InvoiceCategory::Meal;
    }

    InvoiceCategory::Other
}

/// 从高速费发票备注中提取通行时间。
/// 支持格式："YYYY-MM-DD HH:MM:SS" 或 "YYYY-MM-DD"。
/// 取第一个匹配的日期时间字符串。
pub fn extract_toll_travel_time(remarks: &str) -> Option<chrono::NaiveDateTime> {
    // 优先匹配 "YYYY-MM-DD HH:MM:SS"，日期与时间之间空格可选
    // OCR 常将日期和时间粘连，如 "2026-05-2510:06:04"
    let re_datetime = regex::Regex::new(r"(\d{4}-\d{2}-\d{2})\s*(\d{2}:\d{2}:\d{2})").ok()?;
    if let Some(caps) = re_datetime.captures(remarks) {
        let combined = format!("{} {}", &caps[1], &caps[2]);
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&combined, "%Y-%m-%d %H:%M:%S") {
            return Some(dt);
        }
    }
    // 回退匹配 "YYYY-MM-DD"
    let re_date = regex::Regex::new(r"(\d{4}-\d{2}-\d{2})").ok()?;
    if let Some(caps) = re_date.captures(remarks) {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(&caps[1], "%Y-%m-%d") {
            return d.and_hms_opt(0, 0, 0);
        }
    }
    None
}

fn extract_amount(text: &str) -> Result<f64, String> {
    // 多步策略：每个匹配强制要求两位小数，排除整数匹配（如2026、168、税号）

    // Step 0: 数字在关键字前 — "6.30价税合计" / "13.00价税合计"
    // 遍历所有匹配取最大值，跳过税额行（修复华住酒店"143.10\n价税合计"误匹配税额）
    // 使用 [^\S\n]* 替代 \s* 避免跨行匹配（pdfplumber 用 \n 分行，OCR 用空格 join）
    let re_step0 = Regex::new(r"([\d,]+\.\d{2})[^\S\n]*价税合计").map_err(|e| e.to_string())?;
    let mut max_step0 = 0.0f64;
    for cap in re_step0.captures_iter(text) {
        // 检查匹配所在行是否含税额上下文
        let match_start = cap.get(0).unwrap().start();
        let line_start = text[..match_start].rfind('\n').map_or(0, |p| p + 1);
        let context_before = &text[line_start..match_start];
        if context_before.contains("税额") || context_before.contains("税率") || context_before.contains("不含税") {
            continue;
        }
        let v: f64 = cap[1].replace(",", "").parse().unwrap_or(0.0);
        if v > max_step0 && v < 1_000_000.0 {
            max_step0 = v;
        }
    }
    if max_step0 > 0.0 {
        return Ok(max_step0);
    }

    // Step 1: 关键字 + ¥ + 两位小数 — "价税合计（大写） ¥523.57"
    let re_step1 =
        Regex::new(r"(?:价税合计|合计金额|总金额)[^¥￥]{0,20}[¥￥]\s*([\d,]+\.\d{2})")
            .map_err(|e| e.to_string())?;
    if let Some(caps) = re_step1.captures(text) {
        let amount_str = caps[1].replace(",", "");
        return amount_str.parse::<f64>().map_err(|e| e.to_string());
    }

    // Step 2: 关键字后紧邻（10字符内）两位小数 — "价税合计¥6.30"
    let re_step2 =
        Regex::new(r"(?:价税合计|合计金额)[^0-9]{0,10}([\d,]+\.\d{2})")
            .map_err(|e| e.to_string())?;
    if let Some(caps) = re_step2.captures(text) {
        let amount_str = caps[1].replace(",", "");
        return amount_str.parse::<f64>().map_err(|e| e.to_string());
    }

    // 行程单格式：合计XXX.XX元（保留两位小数要求）
    let re_itinerary = Regex::new(r"合计\s*([\d,]+\.\d{2})\s*元").map_err(|e| e.to_string())?;
    if let Some(caps) = re_itinerary.captures(text) {
        let amount_str = caps[1].replace(",", "");
        return amount_str.parse::<f64>().map_err(|e| e.to_string());
    }

    // Step 2.5: 区域内裸两位小数（无¥），取最大值，排除>1e6（税号）
    // 限制数字长度1-7位，避免匹配税号等长数字
    let re_step25 = Regex::new(r"\b([\d,]{1,7}\.\d{2})\b").map_err(|e| e.to_string())?;
    let mut max_bare = 0.0f64;
    for cap in re_step25.captures_iter(text) {
        let v: f64 = cap[1].replace(",", "").parse().unwrap_or(0.0);
        if v > max_bare && v < 1_000_000.0 {
            max_bare = v;
        }
    }
    if max_bare > 0.0 {
        return Ok(max_bare);
    }

    // Step 3: 全文 ¥金额，取最大值（已有逻辑保留，加<1_000_000排除税号）
    let re_yuan = Regex::new(r"[￥¥]\s*([\d,]+\.?\d*)").map_err(|e| e.to_string())?;
    let mut max_amount = 0.0f64;
    for cap in re_yuan.captures_iter(text) {
        let v: f64 = cap[1].replace(",", "").parse().unwrap_or(0.0);
        if v > max_amount && v < 1_000_000.0 {
            max_amount = v;
        }
    }
    if max_amount > 0.0 {
        return Ok(max_amount);
    }

    Err("无法识别发票金额".to_string())
}

fn extract_seller_name(text: &str) -> String {
    // 精确匹配（原逻辑）
    let re = Regex::new(r"名称[：:]\s*(.+?)(?:\s+统一社会信用代码|\s+$)").unwrap();
    if let Some(caps) = re.captures(text) {
        let name = caps[1].trim();
        if !name.is_empty() && name.len() > 2 {
            return name.to_string();
        }
    }
    // 容空格：parangi 在 CJK 字符间插入空格，如"名 称:" → 用 find_iter 找到所有"名称:"位置
    // 手动提取每个候选（regex 不支持 lookahead）
    let re_start = Regex::new(r"名\s*称\s*[：:]").unwrap();
    let re_end = Regex::new(r"\s*(?:名\s*称|统一|纳税人|电话|开户|地址|销|买|售|备)|$").unwrap();
    // 使用 layout_extractor 的统一买方关键词列表（消除重复）
    use crate::parser::layout_extractor::BUYER_KEYWORDS;
    let mut candidates: Vec<String> = Vec::new();
    for m in re_start.find_iter(text) {
        let after = &text[m.end()..];
        let end_pos = re_end.find(after).map(|em| em.start()).unwrap_or(after.len());
        let name = after[..end_pos].trim()
            .trim_end_matches(|c: char| c == '买' || c == '售' || c == ' ');
        if name.len() > 2 && !candidates.iter().any(|c| c == name) {
            candidates.push(name.to_string());
        }
    }
    // 从后往前找第一个非买方候选（卖方通常在买方之后）
    for candidate in candidates.iter().rev() {
        if !BUYER_KEYWORDS.iter().any(|kw| candidate.contains(kw)) {
            return candidate.clone();
        }
    }
    // 全是买方候选，取最后一个
    if let Some(last) = candidates.last() {
        return last.clone();
    }
    // 回退：尝试其他模式
    let re2 = Regex::new(r"(?:销售方|收款单位|开票方)[：:]\s*(.+?)(?:\s|$)").unwrap();
    if let Some(caps) = re2.captures(text) {
        return caps[1].trim().to_string();
    }
    String::new()
}

/// 公司名后缀模式回退：parangi 列交错文本中，正常"名称:"提取得到乱码时，
/// 用公司名后缀（股份有限公司/有限责任公司/有限公司/公司）从全文匹配。
/// 取最后一个匹配——销售方通常在买方之后，且买方常为非公司主体（个人/大学）。
/// ponytail: 启发式，买方也是公司时可能取错；升级路径=用坐标区分买/卖方列。
fn extract_company_name_fallback(text: &str) -> Option<String> {
    let re = Regex::new(
        r"([\u4e00-\u9fa5（）()]{2,40}(?:股份有限公司|有限责任公司|有限公司|公司))",
    )
    .unwrap();
    re.captures_iter(text)
        .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        .filter(|n| n.chars().count() >= 3)
        .last()
}

fn extract_seller_by_coords(texts: &[OcrTextItem]) -> String {
    let re = Regex::new(r"名称[：:]\s*(\S.+)").unwrap();
    let mut best_x = 0.0f64;
    let mut best_name = String::new();
    for item in texts {
        if let Some(caps) = re.captures(&item.text) {
            let name = caps[1].trim();
            // 修复：使用 chars().count() 而非 len()（字节计数），
            // 中文字符 3 字节但 1 字符，"公司A" = 7 字节但 4 字符
            if name.chars().count() <= 2 { continue; }
            if let Some(coords) = &item.box_coords {
                // 修复：使用 X 中心而非 X0（左边缘），
                // 宽 Word 跨两栏时左边缘可能比买方还左，但中心点在右栏
                if let Some(pts) = coords.get("points").and_then(|p| p.as_array()) {
                    let xs: Vec<f64> = pts.iter().filter_map(|p| p.get("x").and_then(|v| v.as_f64())).collect();
                    if !xs.is_empty() {
                        let x_center = (xs.iter().cloned().fold(f64::INFINITY, f64::min)
                            + xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) / 2.0;
                        if x_center > best_x {
                            best_x = x_center;
                            best_name = name.to_string();
                        }
                    }
                }
            }
        }
    }
    best_name
}

/// 从 box_coords 提取顶部 Y 坐标（points[0].y）
fn box_top_y(coords: &Option<serde_json::Value>) -> Option<f64> {
    coords.as_ref()?
        .get("points")?
        .as_array()?
        .first()?
        .get("y")?
        .as_f64()
}

/// 从 box_coords 提取底部 Y 坐标（points[2].y）
fn box_bottom_y(coords: &Option<serde_json::Value>) -> Option<f64> {
    coords.as_ref()?
        .get("points")?
        .as_array()?
        .get(2)?
        .get("y")?
        .as_f64()
}

/// 通行费发票"备注"二字常为竖排印刷，OCR 识别不到，
/// 导致 split_into_regions 无法切换到 remarks 区域。
/// 此函数通过坐标从"价税合计"下方、"开票人"上方恢复备注文本。
fn extract_toll_remarks_by_coords(texts: &[OcrTextItem]) -> String {
    // 找价税合计行的底部 Y（备注在其下方）
    let total_bottom_y = texts.iter()
        .filter(|t| t.text.contains("价税合计"))
        .filter_map(|t| box_bottom_y(&t.box_coords))
        .max_by(|a, b| a.partial_cmp(b).unwrap());

    let total_bottom_y = match total_bottom_y {
        Some(y) => y,
        None => return String::new(),
    };

    // 找开票人行的顶部 Y（备注在其上方，若存在）
    let drawer_top_y = texts.iter()
        .filter(|t| t.text.contains("开票人"))
        .filter_map(|t| box_top_y(&t.box_coords))
        .min_by(|a, b| a.partial_cmp(b).unwrap());

    // 收集价税合计下方、开票人上方（若有）的文本，按 Y 坐标排序
    let mut parts: Vec<(f64, String)> = Vec::new();
    for item in texts {
        let y = match box_top_y(&item.box_coords) {
            Some(y) => y,
            None => continue,
        };
        if y <= total_bottom_y {
            continue;
        }
        if let Some(drawer_y) = drawer_top_y {
            if y >= drawer_y {
                continue;
            }
        }
        let text = item.text.trim();
        if text.is_empty() {
            continue;
        }
        // 排除页脚噪声
        if text.contains("localhost") || text == "1/1" {
            continue;
        }
        parts.push((y, text.to_string()));
    }

    parts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    parts.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(" ")
}

fn extract_item_name(text: &str) -> String {
    // 从商品明细区域提取项目名称
    // 匹配 *服务类型* 格式
    let re_star = Regex::new(r"\*(.+?)\*").unwrap();
    if let Some(caps) = re_star.captures(text) {
        return caps[1].to_string();
    }
    // 回退：尝试其他模式
    let re = Regex::new(r"(?:项目名称|货物或应税劳务|商品名称)[：:]\s*(.+?)(?:\s|$)").unwrap();
    if let Some(caps) = re.captures(text) {
        return caps[1].trim().to_string();
    }
    String::new()
}

fn extract_date(text: &str) -> chrono::NaiveDate {
    // 四字年份："2026年05月06日"
    let re = Regex::new(r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日").unwrap();
    if let Some(caps) = re.captures(text) {
        let y: i32 = caps[1].parse().unwrap_or(2025);
        let m: u32 = caps[2].parse().unwrap_or(1);
        let d: u32 = caps[3].parse().unwrap_or(1);
        return chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default();
    }
    // 两字年份："20年06月05日" → 2000 + 20 = 2020
    let re_short = Regex::new(r"(\d{2})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日").unwrap();
    if let Some(caps) = re_short.captures(text) {
        let y: i32 = 2000 + caps[1].parse::<i32>().unwrap_or(25);
        let m: u32 = caps[2].parse().unwrap_or(1);
        let d: u32 = caps[3].parse().unwrap_or(1);
        return chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default();
    }
    let re2 = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap();
    if let Some(caps) = re2.captures(text) {
        let y: i32 = caps[1].parse().unwrap_or(2025);
        let m: u32 = caps[2].parse().unwrap_or(1);
        let d: u32 = caps[3].parse().unwrap_or(1);
        return chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default();
    }
    chrono::NaiveDate::default()
}

fn extract_invoice_number(text: &str) -> String {
    // 正常模式：发票号码：12345678
    let re = Regex::new(r"(?:发票号码|No)[：:]\s*(\d+)").unwrap();
    if let Some(caps) = re.captures(text) {
        return caps[1].to_string();
    }
    // 容空格模式：pdfplumber 中 CJK 间有空格如"发 票 号 码:32092584"
    let re_space = Regex::new(r"发\s*票\s*号\s*码[：:]?\s*(\d+)").unwrap();
    if let Some(caps) = re_space.captures(text) {
        return caps[1].to_string();
    }
    // 反向模式：PDF文字提取时列顺序可能颠倒，号码出现在标签之前
    // 例如：...26512000001728418261发票号码：...
    let re_rev = Regex::new(r"(\d{8,20})\s*发票号码").unwrap();
    if let Some(caps) = re_rev.captures(text) {
        return caps[1].to_string();
    }
    // 反向容空格
    let re_rev_space = Regex::new(r"(\d{8,20})\s*发\s*票\s*号\s*码").unwrap();
    if let Some(caps) = re_rev_space.captures(text) {
        return caps[1].to_string();
    }
    String::new()
}

pub fn classify_invoice(seller_name: &str, item_name: &str) -> InvoiceCategory {
    let combined = format!("{} {}", seller_name, item_name);
    let combined_lower = combined.to_lowercase();

    if contains_any(&combined_lower, &["铁路", "高铁", "火车", "客运站"]) {
        InvoiceCategory::Train
    } else if contains_any(&combined_lower, &["保险", "意外险"]) {
        InvoiceCategory::Insurance
    } else if contains_any(&combined_lower, &["退票", "改签"]) {
        InvoiceCategory::TicketChange
    } else if contains_any(&combined_lower, &["航空", "机票", "机场", "航班"]) {
        InvoiceCategory::Flight
    } else if contains_any(
        &combined_lower,
        &["出租", "网约车", "滴滴", "高德", "t3", "曹操"],
    ) {
        InvoiceCategory::CityTransport
    } else if contains_any(&combined_lower, &["酒店", "宾馆", "住宿", "招待所", "民宿"]) {
        InvoiceCategory::Hotel
    } else if contains_any(&combined_lower, &["通行", "etc"]) {
        InvoiceCategory::Toll
    } else if contains_any(&combined_lower, &["餐饮", "饭店", "食品", "餐厅", "饭馆"]) {
        InvoiceCategory::Meal
    } else {
        InvoiceCategory::Other
    }
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|k| text.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::structured_output::{BoundingBox, OcrTextBlock, PageLayout, TextBlockType};

    fn create_ocr_output(texts: Vec<&str>) -> OcrStructuredOutput {
        let blocks = texts
            .iter()
            .enumerate()
            .map(|(i, text)| OcrTextBlock {
                text: text.to_string(),
                confidence: 0.95,
                bbox: BoundingBox {
                    x: 0.0,
                    y: (i * 20) as f64,
                    width: 200.0,
                    height: 20.0,
                },
                line_index: i,
                block_type: if text.contains("：") {
                    TextBlockType::KeyValue
                } else {
                    TextBlockType::Other
                },
            })
            .collect();

        OcrStructuredOutput {
            blocks,
            layout: PageLayout {
                width: 600.0,
                height: 1000.0,
                text_regions: vec![],
            },
        }
    }

    #[test]
    fn test_classify_from_full_text_with_tax_code() {
        let ocr = create_ocr_output(vec!["*住宿服务*", "金额：500.00"]);
        let result = classify_from_full_text(&ocr, &InvoiceType::Other);
        assert_eq!(result, InvoiceCategory::Hotel);
    }

    #[test]
    fn test_classify_from_full_text_flight() {
        let ocr = create_ocr_output(vec!["机票行程单", "航班号：CA1234"]);
        let result = classify_from_full_text(&ocr, &InvoiceType::FlightInvoice);
        assert_eq!(result, InvoiceCategory::Flight);
    }

    #[test]
    fn test_classify_insurance_invoice_as_insurance() {
        // 机票保险费发票应分类为 Insurance，而非 Flight 或 TicketChange
        // 真实样本：项目名称"*保险服务*国内机票航空意外险"
        let ocr = create_ocr_output(vec![
            "电子发票（普通发票）",
            "*保险服务*国内机票航空意外险",
            "众安在线财产保险股份有限公司",
            "价税合计：¥50.00",
        ]);
        let result =
            classify_from_full_text(&ocr, &InvoiceType::VatElectronicInvoice);
        assert_eq!(result, InvoiceCategory::Insurance);
    }

    #[test]
    fn test_classify_insurance_invoice_defense_against_flight_type() {
        // 防御性测试：即使 InvoiceType 被误判为 FlightInvoice，
        // 含"保险"的发票仍应归为 Insurance
        let ocr = create_ocr_output(vec![
            "*保险服务*国内机票航空意外险",
            "价税合计：¥50.00",
        ]);
        let result = classify_from_full_text(&ocr, &InvoiceType::FlightInvoice);
        assert_eq!(result, InvoiceCategory::Insurance);
    }

    #[test]
    fn test_classify_from_regions_insurance_not_flight() {
        // 众安保险发票：items_text 含"*保险服务*国内机票航空意外"
        // classify_from_regions 应优先识别"保险服务"→Insurance，而非"机票"→Flight
        // 真实样本：20/21_电子发票（众安在线财产保险）
        let items_text = "*保险服务*国内机票航空意外 ** 1 47.169811 47.17 6% 2.83";
        let seller_text = "众安在线财产保险股份有限公司";
        let item_name = "保险服务";
        let seller_name = "众安在线财产保险股份有限公司";
        let result = classify_from_regions(items_text, seller_text, item_name, seller_name);
        assert_eq!(result, InvoiceCategory::Insurance);
    }

    #[test]
    fn test_classify_from_full_text_city_transport() {
        let ocr = create_ocr_output(vec!["滴滴出行电子发票", "网约车服务"]);
        let result =
            classify_from_full_text(&ocr, &InvoiceType::RideHailingInvoice);
        assert_eq!(result, InvoiceCategory::CityTransport);
    }

    #[test]
    fn test_classify_from_full_text_keywords() {
        let ocr = create_ocr_output(vec!["如家酒店住宿费", "金额：300.00"]);
        let result = classify_from_full_text(&ocr, &InvoiceType::Other);
        assert_eq!(result, InvoiceCategory::Hotel);
    }

    #[test]
    fn test_backward_compatibility() {
        let texts = vec![
            OcrTextItem {
                text: "发票号码：12345678".to_string(),
                confidence: 0.99,
                box_coords: None,
            },
            OcrTextItem {
                text: "价税合计：¥200.00".to_string(),
                confidence: 0.99,
                box_coords: None,
            },
            OcrTextItem {
                text: "销售方：滴滴出行".to_string(),
                confidence: 0.99,
                box_coords: None,
            },
        ];
        let result = parse_invoice_text(&texts, InvoiceSource::Photo("test.jpg".to_string()));
        assert!(result.is_ok());
        let invoice = result.unwrap();
        assert_eq!(invoice.invoice_number, "12345678");
        assert!((invoice.amount - 200.0).abs() < 0.01);
        assert_eq!(invoice.seller_name, "滴滴出行");
    }

    #[test]
    fn test_classify_train() {
        assert!(matches!(
            classify_invoice("中国铁路", ""),
            InvoiceCategory::Train
        ));
        assert!(matches!(
            classify_invoice("", "高铁票"),
            InvoiceCategory::Train
        ));
    }

    #[test]
    fn test_classify_flight() {
        assert!(matches!(
            classify_invoice("中国航空", ""),
            InvoiceCategory::Flight
        ));
    }

    #[test]
    fn test_classify_insurance_as_insurance() {
        // 机票保险费发票应分类为 Insurance，而非 Flight 或 TicketChange
        // 真实样本：销售方"众安在线财产保险"，项目"国内机票航空意外险"
        assert!(matches!(
            classify_invoice("众安在线财产保险股份有限公司", "国内机票航空意外险"),
            InvoiceCategory::Insurance
        ));
        assert!(matches!(
            classify_invoice("", "航空意外险"),
            InvoiceCategory::Insurance
        ));
    }

    #[test]
    fn test_classify_hotel() {
        assert!(matches!(
            classify_invoice("如家酒店", ""),
            InvoiceCategory::Hotel
        ));
    }

    #[test]
    fn test_classify_taxi() {
        assert!(matches!(
            classify_invoice("滴滴出行", ""),
            InvoiceCategory::CityTransport
        ));
    }

    #[test]
    fn test_extract_amount() {
        let text = "价税合计：¥553.00";
        assert_eq!(extract_amount(text).unwrap(), 553.0);
    }

    #[test]
    fn test_extract_amount_with_comma() {
        let text = "价税合计：¥1,234.56";
        assert!((extract_amount(text).unwrap() - 1234.56).abs() < 0.01);
    }

    #[test]
    fn test_extract_date_cn() {
        let text = "2025年08月05日";
        let date = extract_date(text);
        assert_eq!(
            date,
            chrono::NaiveDate::from_ymd_opt(2025, 8, 5).unwrap()
        );
    }

    #[test]
    fn test_contains_any() {
        assert!(contains_any("滴滴出行", &["滴滴", "高德"]));
        assert!(!contains_any("出租车", &["滴滴", "高德"]));
        assert!(contains_any("hello", &["hello"]));
        assert!(!contains_any("world", &["hello"]));
    }

    #[test]
    fn test_classify_from_full_text_train_with_transport_service_tax_code() {
        // 火车票增值税发票使用 *运输服务* 税收编码，不应误识别为 CityTransport
        let ocr = create_ocr_output(vec!["*运输服务*", "中国铁路", "高铁", "金额：200.00"]);
        let result = classify_from_full_text(&ocr, &InvoiceType::TrainInvoice);
        assert_eq!(result, InvoiceCategory::Train);
    }

    #[test]
    fn test_classify_from_full_text_train_with_passenger_tax_code() {
        // 火车票使用 *客运服务* 税收编码
        let ocr = create_ocr_output(vec!["*客运服务*", "铁路", "金额：150.00"]);
        let result = classify_from_full_text(&ocr, &InvoiceType::TrainInvoice);
        assert_eq!(result, InvoiceCategory::Train);
    }

    #[test]
    fn test_classify_from_regions_train_with_transport_service_tax_code() {
        // 验证 classify_from_regions 中 *运输服务* 的火车票不被误识别为 CityTransport
        let items_text = "*运输服务*高铁票 1 200.00";
        let seller_text = "名称：中国铁路成都局";
        let result = classify_from_regions(items_text, seller_text, "", "中国铁路成都局");
        assert_eq!(result, InvoiceCategory::Train);
    }

    #[test]
    fn test_extract_ticket_cities_train() {
        let text = "出发站：北京南站\n到达站：上海虹桥站\n票价：553.00";
        let (dep, arr) = extract_ticket_cities(text, &InvoiceCategory::Train);
        assert_eq!(dep.as_deref(), Some("北京"));
        assert_eq!(arr.as_deref(), Some("上海"));
    }

    #[test]
    fn test_extract_ticket_cities_flight() {
        let text = "自：北京首都国际机场\n至：上海浦东国际机场\n航班号：CA1234";
        let (dep, arr) = extract_ticket_cities(text, &InvoiceCategory::Flight);
        assert_eq!(dep.as_deref(), Some("北京"));
        assert_eq!(arr.as_deref(), Some("上海"));
    }

    #[test]
    fn test_extract_ticket_cities_no_keyword() {
        let text = "这是普通的住宿发票";
        let (dep, arr) = extract_ticket_cities(text, &InvoiceCategory::Hotel);
        assert!(dep.is_none());
        assert!(arr.is_none());
    }

    #[test]
    fn test_station_to_city_suffix_strip() {
        assert_eq!(station_to_city("上海虹桥站"), "上海");
        assert_eq!(station_to_city("广州南站"), "广州");
        assert_eq!(station_to_city("成都双流国际机场"), "成都");
    }

    #[test]
    fn test_station_to_city_mapping() {
        assert_eq!(station_to_city("虹桥"), "上海");
        assert_eq!(station_to_city("宝安"), "深圳");
    }

    #[test]
    fn test_extract_toll_travel_time_standard_format() {
        let remarks = "湘ADG5926 湖南新港站入 湖南黄花站出 2026-05-25 10:06:04 （不可用于增值税进项抵扣）";
        let time = extract_toll_travel_time(remarks);
        assert!(time.is_some());
        let t = time.unwrap();
        assert_eq!(t.date(), chrono::NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
        assert_eq!(t.time(), chrono::NaiveTime::from_hms_opt(10, 6, 4).unwrap());
    }

    #[test]
    fn test_extract_toll_travel_time_second_example() {
        let remarks = "川AB55365 四川天府机场T1T2站入 四川天府机场成都站出 2026-06-23 14:24:10 （不可用于增值税进项抵扣）";
        let time = extract_toll_travel_time(remarks);
        assert!(time.is_some());
        let t = time.unwrap();
        assert_eq!(t.date(), chrono::NaiveDate::from_ymd_opt(2026, 6, 23).unwrap());
        assert_eq!(t.time(), chrono::NaiveTime::from_hms_opt(14, 24, 10).unwrap());
    }

    #[test]
    fn test_extract_toll_travel_time_no_date() {
        let remarks = "普通备注无时间";
        let time = extract_toll_travel_time(remarks);
        assert!(time.is_none());
    }

    #[test]
    fn test_extract_toll_travel_time_date_only() {
        let remarks = "通行时间 2026-05-25";
        let time = extract_toll_travel_time(remarks);
        assert!(time.is_some());
        let t = time.unwrap();
        assert_eq!(t.date(), chrono::NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
        assert_eq!(t.time(), chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    }

    /// Bug: OCR 将日期和时间粘连（无空格），如 "2026-05-2510:06:04"
    #[test]
    fn test_extract_toll_travel_time_no_space_between_date_time() {
        let remarks = "湘ADG5926 湖南新港站入湖南黄花站出2026-05-2510:06:04（不可用于增值税进项抵扣）";
        let time = extract_toll_travel_time(remarks);
        assert!(time.is_some(), "should extract time even without space");
        let t = time.unwrap();
        assert_eq!(t.date(), chrono::NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
        assert_eq!(t.time(), chrono::NaiveTime::from_hms_opt(10, 6, 4).unwrap());
    }

    /// Bug: 通行费发票"备注"二字竖排印刷，OCR 识别不到，
    /// 导致 split_into_regions 无法切换到 remarks 区域，备注内容丢失。
    /// 应通过坐标从价税合计下方恢复备注。
    // ===== extract_amount TDD tests =====

    #[test]
    fn test_extract_amount_tianfutong_not_2026() {
        // Bug: #3 天府通13元 — ¥13.00价税合计...2026 should not return 2026
        let text = "壹拾叁圆整¥13.00价税合计（大写） （小写） 2026/04/24-2026/04/26";
        let result = extract_amount(text).unwrap();
        assert!((result - 13.00).abs() < 0.01, "expected 13.00, got {}", result);
    }

    #[test]
    fn test_extract_amount_before_keyword() {
        // Bug: #1 长沙轨交 pdfplumber — "6.30价税合计"
        let text = "6.30价税合计(大写) ¥ 陆圆叁角整 (小写)";
        let result = extract_amount(text).unwrap();
        assert!((result - 6.30).abs() < 0.01, "expected 6.30, got {}", result);
    }

    #[test]
    fn test_extract_amount_exclude_taxid() {
        // Bug: tax ID "91430100578607044B" should not be captured
        let text = "91430100578607044B 价税合计 ¥6.30";
        let result = extract_amount(text).unwrap();
        assert!((result - 6.30).abs() < 0.01, "expected 6.30, got {}", result);
    }

    #[test]
    fn test_extract_amount_normal_jiaoshuiheji() {
        // Normal amount with Chinese amount words
        let text = "价税合计（大写） （小写）伍佰贰拾叁圆伍角柒分 ¥523.57";
        let result = extract_amount(text).unwrap();
        assert!((result - 523.57).abs() < 0.01, "expected 523.57, got {}", result);
    }

    // ===== extract_seller_name TDD tests =====

    #[test]
    fn test_extract_seller_name_with_spaces() {
        // Bug: parangi inserts spaces in CJK text "名 称:"
        let text = "名 称: 长沙市轨道交通运营有限公司销 备纳税人识别号";
        let result = extract_seller_name(text);
        assert_eq!(result, "长沙市轨道交通运营有限公司");
    }

    #[test]
    fn test_extract_seller_name_double_name_take_seller() {
        // Bug: two "名称:" entries — must exclude buyer (国防大学)
        let text = "名称：中国人民解放军国防科技大学 名称：成都滴滴优行科技有限公司买 售";
        let result = extract_seller_name(text);
        assert_eq!(result, "成都滴滴优行科技有限公司");
    }

    #[test]
    fn test_extract_seller_name_normal_single() {
        // Normal single name entry
        let text = "名称：四川景澜酒店管理有限公司 统一社会信用代码";
        let result = extract_seller_name(text);
        assert_eq!(result, "四川景澜酒店管理有限公司");
    }

    // ===== extract_date TDD tests =====

    #[test]
    fn test_extract_date_normal_four_digit_year() {
        let text = "开票日期:2026年05月06日";
        let date = extract_date(text);
        assert_eq!(date, chrono::NaiveDate::from_ymd_opt(2026, 5, 6).unwrap());
    }

    #[test]
    fn test_extract_date_two_digit_year() {
        // Bug: #1 pdfplumber — "20年 6 月 日 05 06" → year "20" needs 2000+ prefix
        let text = "20年06月05日";
        let date = extract_date(text);
        assert_eq!(date, chrono::NaiveDate::from_ymd_opt(2020, 6, 5).unwrap());
    }

    // ===== extract_invoice_number TDD tests =====

    #[test]
    fn test_extract_invoice_number_with_spaces() {
        // Bug: pdfplumber "发 票 号 码:" with spaces between CJK
        let text = "发 票 号 码:32092584";
        let result = extract_invoice_number(text);
        assert_eq!(result, "32092584");
    }

    #[test]
    fn test_extract_invoice_number_normal() {
        let text = "发票号码:26517000000358455168";
        let result = extract_invoice_number(text);
        assert_eq!(result, "26517000000358455168");
    }

    #[test]
    fn test_parse_toll_invoice_remarks_recovered_by_coords() {
        let texts = vec![
            OcrTextItem {
                text: "发票号码：2643700002859951".to_string(),
                confidence: 0.99,
                box_coords: Some(serde_json::json!({
                    "points":[{"x":1177,"y":131},{"x":1524,"y":131},{"x":1524,"y":176},{"x":1177,"y":176}]
                })),
            },
            OcrTextItem {
                text: "开票日期：2026年06月07日".to_string(),
                confidence: 1.0,
                box_coords: Some(serde_json::json!({
                    "points":[{"x":1177,"y":199},{"x":1456,"y":199},{"x":1456,"y":237},{"x":1177,"y":237}]
                })),
            },
            OcrTextItem {
                text: "名称：中国人民解放军国防科技大学".to_string(),
                confidence: 1.0,
                box_coords: Some(serde_json::json!({
                    "points":[{"x":170,"y":321},{"x":552,"y":321},{"x":552,"y":376},{"x":170,"y":376}]
                })),
            },
            OcrTextItem {
                text: "名称：湖南省高速公路集团有限公司".to_string(),
                confidence: 1.0,
                box_coords: Some(serde_json::json!({
                    "points":[{"x":882,"y":326},{"x":1251,"y":326},{"x":1251,"y":371},{"x":882,"y":371}]
                })),
            },
            OcrTextItem {
                text: "*生产生活服务*通行费".to_string(),
                confidence: 1.0,
                box_coords: Some(serde_json::json!({
                    "points":[{"x":131,"y":521},{"x":376,"y":521},{"x":376,"y":566},{"x":131,"y":566}]
                })),
            },
            OcrTextItem {
                text: "价税合计（大写）".to_string(),
                confidence: 0.98,
                box_coords: Some(serde_json::json!({
                    "points":[{"x":202,"y":769},{"x":388,"y":769},{"x":388,"y":816},{"x":202,"y":816}]
                })),
            },
            OcrTextItem {
                text: "￥12.00".to_string(),
                confidence: 0.92,
                box_coords: Some(serde_json::json!({
                    "points":[{"x":1192,"y":769},{"x":1286,"y":769},{"x":1286,"y":819},{"x":1192,"y":819}]
                })),
            },
            // 备注行：无"备注"关键词，在价税合计下方
            OcrTextItem {
                text: "湘ADG5926 湖南新港站入湖南黄花站出2026-05-2510:06:04（不可用于增值税进项抵扣）".to_string(),
                confidence: 0.97,
                box_coords: Some(serde_json::json!({
                    "points":[{"x":172,"y":838},{"x":1097,"y":838},{"x":1097,"y":883},{"x":172,"y":883}]
                })),
            },
            OcrTextItem {
                text: "开票人：刘婷婷".to_string(),
                confidence: 1.0,
                box_coords: Some(serde_json::json!({
                    "points":[{"x":114,"y":989},{"x":293,"y":989},{"x":293,"y":1034},{"x":114,"y":1034}]
                })),
            },
        ];
        let result = parse_invoice_text(&texts, InvoiceSource::Pdf("toll.pdf".to_string()));
        assert!(result.is_ok());
        let inv = result.unwrap();
        assert_eq!(inv.category, InvoiceCategory::Toll);
        assert!(
            inv.remarks.contains("湘ADG5926"),
            "remarks should contain plate number, got: '{}'",
            inv.remarks
        );
        assert!(
            inv.toll_travel_time.is_some(),
            "toll_travel_time should be extracted from recovered remarks"
        );
        let t = inv.toll_travel_time.unwrap();
        assert_eq!(t.date(), chrono::NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
        assert_eq!(t.time(), chrono::NaiveTime::from_hms_opt(10, 6, 4).unwrap());
    }

    #[test]
    fn test_extract_amount_step0_skips_tax_line() {
        // 华住酒店场景：税额"143.10"紧邻"价税合计"标签前，跨行
        // Step0 应跳过税额行，不应返回 143.10
        let text = "*生产生活服务*住宿费 天 7 340.7075 2384.95 6% 143.10\n价税合计（大写） 贰仟伍佰贰拾捌圆零伍分 （小写） ¥ 2528.05";
        let result = extract_amount(text);
        assert!(result.is_ok());
        let amount = result.unwrap();
        assert!((amount - 2528.05).abs() < 0.01, "应提取价税合计 2528.05，而非税额 143.10，实际: {}", amount);
    }

    #[test]
    fn test_extract_amount_step0_ocr_reversed() {
        // OCR 顺序颠倒场景："6.30价税合计"（数字在关键字前，同行）
        // Step0 仍应正确提取
        let text = "6.30价税合计";
        let result = extract_amount(text);
        assert!(result.is_ok());
        let amount = result.unwrap();
        assert!((amount - 6.30).abs() < 0.01, "OCR 颠倒场景应提取 6.30，实际: {}", amount);
    }

    #[test]
    fn test_extract_amount_step0_takes_max() {
        // 多个"数字价税合计"匹配时取最大值
        let text = "税额 143.10价税合计\n实际 2528.05价税合计";
        let result = extract_amount(text);
        assert!(result.is_ok());
        let amount = result.unwrap();
        assert!((amount - 2528.05).abs() < 0.01, "应取最大值 2528.05，实际: {}", amount);
    }

    #[test]
    fn test_classify_invoice_toll() {
        // 票根高速发票应分类为 Toll
        let category = classify_invoice("湖南省高速公路集团有限公司", "通行费");
        assert_eq!(category, InvoiceCategory::Toll);
    }

    #[test]
    fn test_classify_invoice_toll_etc() {
        // ETC 发票应分类为 Toll
        let category = classify_invoice("某ETC公司", "通行");
        assert_eq!(category, InvoiceCategory::Toll);
    }

    #[test]
    fn test_classify_invoice_hotel_not_toll() {
        // "高速公路酒店"含"高速"但应归 Hotel（Toll 用"通行"而非"高速"）
        let category = classify_invoice("高速公路酒店", "住宿服务");
        assert_eq!(category, InvoiceCategory::Hotel);
    }

    #[test]
    fn test_extract_hotel_statement_detail_basic() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        // ISO 横线格式
        let text = "成都九眼桥美居酒店 结账单\n房号 3120\n姓名 陈福旭\n入住日期\n2026-04-27\n离店日期\n2026-04-30\n房费 369.96";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "应该能从入住/离店日期中提取天数");
        let d = detail.unwrap();
        assert_eq!(d.nights, 3, "4-27到4-30 = 3晚");
        assert_eq!(d.check_in.unwrap().to_string(), "2026-04-27");
        assert_eq!(d.check_out.unwrap().to_string(), "2026-04-30");
    }

    #[test]
    fn test_extract_hotel_statement_detail_chinese_format() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        // 中文格式：YYYY年MM月DD日，标签带冒号
        let text = "入住日期：2026年04月27日\n离店日期：2026年04月30日";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "应该支持中文日期格式");
        let d = detail.unwrap();
        assert_eq!(d.nights, 3);
        assert_eq!(d.check_in.unwrap().to_string(), "2026-04-27");
    }

    #[test]
    fn test_extract_hotel_statement_detail_slash_format() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        // 斜线格式：YYYY/MM/DD
        let text = "入住日期:2026/04/27\n离店日期:2026/04/30";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "应该支持斜线日期格式");
        let d = detail.unwrap();
        assert_eq!(d.nights, 3);
    }

    #[test]
    fn test_extract_hotel_statement_detail_dot_format() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        // 点格式：YYYY.MM.DD
        let text = "入住日期 2026.04.27\n离店日期 2026.04.30";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "应该支持点分隔日期格式");
        let d = detail.unwrap();
        assert_eq!(d.nights, 3);
    }

    #[test]
    fn test_extract_hotel_statement_detail_chinese_compact() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        // 中文紧凑格式（无空格）：YYYY年M月D日
        let text = "入住日期2026年4月27日离店日期2026年4月30日";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "应该支持紧凑中文日期");
        let d = detail.unwrap();
        assert_eq!(d.nights, 3);
    }

    #[test]
    fn test_extract_hotel_statement_detail_yearless() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        // 无年份格式：MM-DD、MM/DD、M月D日
        let text = "入住日期\n04-27\n离店日期\n04-30";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "应该支持无年份MM-DD格式");
        let d = detail.unwrap();
        assert_eq!(d.nights, 3);
        assert_eq!(d.check_in.unwrap().to_string(), "2026-04-27");

        // 中文无年份：M月D日
        let text = "入住日期：4月27日\n离店日期：4月30日";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "应该支持无年份M月D日格式");
        let d = detail.unwrap();
        assert_eq!(d.check_in.unwrap().to_string(), "2026-04-27");

        // 斜线无年份：MM/DD
        let text = "入住日期 04/27\n离店日期 04/30";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "应该支持无年份斜线格式");
        let d = detail.unwrap();
        assert_eq!(d.check_in.unwrap().to_string(), "2026-04-27");

        // 点无年份：MM.DD
        let text = "入住日期:04.27\n离店日期:04.30";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "应该支持无年份点格式");
        let d = detail.unwrap();
        assert_eq!(d.check_in.unwrap().to_string(), "2026-04-27");
    }

    #[test]
    fn test_resolve_year_cross_h1_h2() {
        // 发票1月（H1），入住12月（H2）→ 前一年
        let date = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
        let text = "入住日期\n12-28\n离店日期\n12-31";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "1月发票+12月入住应推断为前一年");
        let d = detail.unwrap();
        assert_eq!(d.check_in.unwrap().year(), 2025, "应推断为2025-12-28");
        assert_eq!(d.nights, 3);

        // 发票12月（H2），入住1月（H1）→ 后一年
        let date = NaiveDate::from_ymd_opt(2026, 12, 5).unwrap();
        let text = "入住日期\n01-05\n离店日期\n01-08";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_some(), "12月发票+1月入住应推断为后一年");
        let d = detail.unwrap();
        assert_eq!(d.check_in.unwrap().year(), 2027, "应推断为2027-01-05");
        assert_eq!(d.nights, 3);
    }

    #[test]
    fn test_resolve_year_same_half() {
        // 同半边直接使用发票年份
        // 发票3月（H1），入住4月（H1）
        let date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        assert_eq!(resolve_year_for_stay(4, date), 2026);
        // 发票8月（H2），入住10月（H2）
        let date = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        assert_eq!(resolve_year_for_stay(10, date), 2026);
        // 边界：发票6月（H1），入住6月（H1）
        let date = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        assert_eq!(resolve_year_for_stay(6, date), 2026);
        // 边界：发票7月（H2），入住7月（H2）
        let date = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        assert_eq!(resolve_year_for_stay(7, date), 2026);
    }

    #[test]
    fn test_parse_nights_from_remarks_with_spaces() {
        // OCR 分行后 "共 3 天"（含空格）
        assert_eq!(parse_nights_from_remarks("成都景澜美居酒店,订单日期:4-24至4-27,共 3 天"), Some(3));
        // 紧凑格式 "共3天"
        assert_eq!(parse_nights_from_remarks("备注:共3天,共1间"), Some(3));
        // 跨行空格 "共\n3\n天" → join后 "共 3 天"
        assert_eq!(parse_nights_from_remarks("订单姓名:陈福旭 共 3 天 共1间"), Some(3));
    }

    #[test]
    fn test_extract_item_quantity_formats() {
        // 标准格式：quantity=3（多天，可信）
        assert_eq!(extract_item_quantity("*住宿服务*住宿费  3  1260.00"), Some(3));
        // 单行项目：quantity=1（不可信）
        assert_eq!(extract_item_quantity("*住宿服务*住宿费  1  420.00"), Some(1));
        // 华住格式：含 "天" 单位
        assert_eq!(extract_item_quantity("*住宿服务*住宿费 天 7 340.70 2384.95"), Some(7));
        // 景澜美居：税法编码 *生产生活服务*（非 *住宿服务*）
        assert_eq!(extract_item_quantity("*生产生活服务*住宿费 天 7 340.707547169811 2384.95 6% 143.10"), Some(7));
    }

    #[test]
    fn test_extract_hotel_statement_detail_no_dates() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        // 标准发票无 入住日期/离店日期 字段，应返回 None
        let text = "*住宿服务*住宿费  1  420.00\n备注:成都景澜美居酒店,共1天";
        let detail = extract_hotel_statement_detail(text, date);
        assert!(detail.is_none(), "标准发票不应匹配结账单日期模式");
    }

    #[test]
    fn test_parse_hotel_detail_generic_prefix() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
        // 不依赖特定前缀，直接匹配 M-DD至M-DD
        let detail = parse_hotel_detail("成都酒店,入住时间:6-1至6-4,共3天", date);
        assert!(detail.is_some(), "应匹配入住时间");
        assert_eq!(detail.unwrap().nights, 3);

        let detail = parse_hotel_detail("酒店住宿期间:3-15至3-19,共4天", date);
        assert!(detail.is_some(), "应匹配住宿期间");
        assert_eq!(detail.unwrap().nights, 4);

        // 电话不应误匹配（无"至"分隔符）
        let detail = parse_hotel_detail("电话:028-87751288,酒店:5-29至6-5", date);
        assert!(detail.is_some(), "应跳过电话号匹配日期");
        assert_eq!(detail.unwrap().nights, 7);
    }

    #[test]
    fn test_parse_invoice_text_hotel_statement_nights() {
        // 模拟酒店结账单 OCR 输出，验证 parse_invoice_text 能正确提取天数
        let texts = vec![
            OcrTextItem {
                text: "成都九眼桥美居酒店 结账单".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
            OcrTextItem {
                text: "房号 3120".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
            OcrTextItem {
                text: "姓名 陈福旭".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
            OcrTextItem {
                text: "入住日期 2026-04-27".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
            OcrTextItem {
                text: "离店日期 2026-04-30".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
            OcrTextItem {
                text: "价税合计 1109.89".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
            OcrTextItem {
                text: "销售方 成都九眼桥美居酒店".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
        ];
        let result = parse_invoice_text(&texts, InvoiceSource::Pdf("hotel.pdf".to_string()));
        assert!(result.is_ok(), "parse_invoice_text should succeed");
        let inv = result.unwrap();
        assert_eq!(inv.category, InvoiceCategory::Hotel);
        let hd = inv.hotel_detail.expect("should have hotel_detail");
        assert_eq!(hd.nights, 3, "酒店结账单应提取3晚，实际: {}", hd.nights);
        assert!(hd.check_in.is_some());
        assert!(hd.check_out.is_some());
    }

    #[test]
    fn test_parse_invoice_text_hotel_quantity_1_not_nights() {
        // 标准税票：items行数量=1，备注含订单日期但无"共N天"
        // 应通过订单日期计算天数(3晚)，不使用item_quantity=1
        let texts = vec![
            OcrTextItem { text: "发票号码:12345678".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "开票日期:2026年05月01日".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "项目名称".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "*住宿服务*住宿费 1 1260.00".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "价税合计 1260.00".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "销售方 成都景澜美居酒店".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "备注".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "成都景澜美居酒店,订单日期:4-27至4-30,共1间,订单姓名:陈福旭".to_string(), confidence: 1.0, box_coords: None },
        ];
        let result = parse_invoice_text(&texts, InvoiceSource::Pdf("hotel.pdf".to_string()));
        assert!(result.is_ok());
        let inv = result.unwrap();
        assert_eq!(inv.category, InvoiceCategory::Hotel);
        let hd = inv.hotel_detail.expect("should have hotel_detail");
        assert_eq!(hd.nights, 3, "应通过订单日期计算=3晚，不能把item_quantity=1当成天数");
    }

    #[test]
    fn test_parse_invoice_text_jinglan_meiju_production_life_service() {
        // 模拟景澜美居发票（税码 *生产生活服务*，备注行含冒号）
        // 验证 split_into_regions 能正确捕获 备注区域（即使备注行含冒号）
        // 验证 extract_item_quantity 能匹配 *生产生活服务* 税码
        let texts = vec![
            OcrTextItem { text: "发票号码: 26512000002353016641".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "开票日期: 2026年06月05日".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "购买方 名称: 国防科技大学".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "销售方 名称: 四川景澜酒店管理有限公司".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "项目名称 规格型号 单位 数量 单价 金额 税率/征收率 税额".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "*生产生活服务*住宿费 天 7 340.707547169811 2384.95 6% 143.10".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "价税合计 ¥2528.05".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "备注 购买方地址:- 电话:- 销方地址:成都市 成都景澜美居酒店,订单日期:5-29至6-5,共7天,共1间,订单姓名:陈福旭".to_string(), confidence: 1.0, box_coords: None },
        ];
        let result = parse_invoice_text(&texts, InvoiceSource::Pdf("hotel.pdf".to_string()));
        assert!(result.is_ok(), "parse_invoice_text failed: {:?}", result.err());
        let inv = result.unwrap();
        assert_eq!(inv.category, InvoiceCategory::Hotel, "应分类为酒店");
        let hd = inv.hotel_detail.expect("should have hotel_detail");
        assert_eq!(hd.nights, 7, "景澜美居5-29至6-5应=7晚，实际: {}", hd.nights);
    }

    #[test]
    fn test_parse_invoice_text_jinglan_24_ru_li_date() {
        // 模拟景澜美居发票 #24：入离日期（非订单日期）+ 备注多行含销售方地址
        // 验证 split_into_regions 不会被 "销售方地址" 切出备注区域
        let texts = vec![
            OcrTextItem { text: "发票号码: 26512000002353030546".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "开票日期: 2026年06月05日".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "购买方 名称: 国防科技大学".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "销售方 名称: 四川景澜酒店管理有限公司".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "项目名称 规格型号 单位 数量 单价 金额 税率/征收率 税额".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "*生产生活服务*住宿费 天 4 346.014150943396 1384.06 6% 83.04".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "价税合计 ¥1467.10".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "备注".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "购买方地址:- 电话:-".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "销售方地址:成都市金牛区花牌坊街168号1栋1单元1层6号 电话:028-87751288".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "成都景澜美居酒店,入离日期:5-25至5-29,共4天,共1间,订单姓名:陈福旭".to_string(), confidence: 1.0, box_coords: None },
        ];
        let result = parse_invoice_text(&texts, InvoiceSource::Pdf("hotel2.pdf".to_string()));
        assert!(result.is_ok(), "parse_invoice_text failed: {:?}", result.err());
        let inv = result.unwrap();
        assert_eq!(inv.category, InvoiceCategory::Hotel, "应分类为酒店");
        let hd = inv.hotel_detail.expect("should have hotel_detail");
        assert_eq!(hd.nights, 4, "#24:5-25至5-29应=4晚，实际: {}", hd.nights);
        assert!(hd.check_in.is_some(), "应有入住日期");
        assert!(hd.check_out.is_some(), "应有离店日期");
    }
}
