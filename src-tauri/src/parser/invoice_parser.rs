use crate::models::invoice::{HotelDetail, Invoice, InvoiceCategory, InvoiceSource};
use chrono::{NaiveDate, Datelike};
use crate::ocr::structured_output::OcrStructuredOutput;
use crate::ocr::OcrTextItem;
use crate::parser::field_extractors::{
    AmountExtractor, DateExtractor, ExtractedField, FieldExtractor, InvoiceNumberExtractor,
    ItemNameExtractor, SellerNameExtractor,
};
use crate::parser::invoice_type_detector::{InvoiceType, InvoiceTypeDetector};
use crate::parser::template_manager::{InvoiceTemplate, TemplateManager};
use regex::Regex;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
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
    let lines: Vec<&str> = text.lines().collect();
    let mut current_region = "header";

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 识别区域切换
        if trimmed.contains("购买方") && (trimmed.contains("名称") || trimmed.contains("统一社会信用代码")) {
            current_region = "buyer";
        } else if trimmed.contains("销售方") {
            current_region = "seller";
        } else if trimmed.contains("项目名称") || trimmed.contains("货物或应税劳务") {
            current_region = "items";
        } else if trimmed.contains("价税合计") || trimmed.contains("票价") || trimmed.contains("合计金额") {
            current_region = "total";
        } else if trimmed.contains("备注") && !trimmed.contains("：") {
            current_region = "remarks";
        }

        // 将文本添加到对应区域
        match current_region {
            "header" => {
                regions.header.push_str(trimmed);
                regions.header.push(' ');
            }
            "buyer" => {
                regions.buyer.push_str(trimmed);
                regions.buyer.push(' ');
            }
            "seller" => {
                regions.seller.push_str(trimmed);
                regions.seller.push(' ');
            }
            "items" => {
                regions.items.push_str(trimmed);
                regions.items.push(' ');
            }
            "total" => {
                regions.total.push_str(trimmed);
                regions.total.push(' ');
            }
            "remarks" => {
                regions.remarks.push_str(trimmed);
                regions.remarks.push(' ');
            }
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
    let all_text: String = texts
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let regions = split_into_regions(&all_text);

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
    let item_name = extract_item_name(&regions.items);
    let date = extract_date(&all_text);
    let invoice_number = extract_invoice_number(&regions.header);

    let mut category = classify_from_regions(&regions.items, &regions.seller, &item_name, &seller_name);

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
        category = classify_from_full_text(&ocr_output, &None, &None, &invoice_type);
    }

    let hotel_detail = if category == InvoiceCategory::Hotel {
        let remarks_nights = parse_nights_from_remarks(&regions.remarks);
        let item_quantity = extract_item_quantity(&regions.items);
        let detail = parse_hotel_detail(&regions.remarks, date);
        // 交叉验证：备注天数 vs 商品数量，不一致时取较大值
        let nights = match (remarks_nights, item_quantity) {
            (Some(r), Some(q)) if r != q => r.max(q),
            (Some(r), _) => r,
            (_, Some(q)) => q,
            _ => detail.as_ref().map(|d| d.nights).unwrap_or(1),
        };
        Some(HotelDetail {
            nights,
            ..detail.unwrap_or(HotelDetail {
                check_in: None,
                check_out: None,
                nights,
                nightly_rate: amount / nights.max(1) as f64,
            })
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
/// 备注格式示例: "成都景澜美居酒店,订单日期:4-24至4-27,共3天,共1间,订单姓名:陈福旭"
fn parse_hotel_detail(remarks: &str, invoice_date: NaiveDate) -> Option<HotelDetail> {
    let year = invoice_date.year();

    // 解析 "订单日期:M-DD至M-DD"
    let date_re = Regex::new(r"订单日期[:：]?\s*(\d{1,2})-(\d{1,2})\s*至\s*(\d{1,2})-(\d{1,2})").ok()?;
    let caps = date_re.captures(remarks)?;
    let in_month: u32 = caps.get(1)?.as_str().parse().ok()?;
    let in_day: u32 = caps.get(2)?.as_str().parse().ok()?;
    let out_month: u32 = caps.get(3)?.as_str().parse().ok()?;
    let out_day: u32 = caps.get(4)?.as_str().parse().ok()?;

    let check_in = NaiveDate::from_ymd_opt(year, in_month, in_day)?;
    let check_out = NaiveDate::from_ymd_opt(year, out_month, out_day)?;
    let nights = (check_out - check_in).num_days().max(1) as usize;

    Some(HotelDetail {
        check_in: Some(check_in),
        check_out: Some(check_out),
        nights,
        nightly_rate: 0.0, // 后续由 form_builder 计算
    })
}

/// 从备注栏解析 "共N天" 获取天数
fn parse_nights_from_remarks(remarks: &str) -> Option<usize> {
    let re = Regex::new(r"共(\d+)天").ok()?;
    let caps = re.captures(remarks)?;
    caps.get(1)?.as_str().parse().ok()
}

/// 从商品明细区域提取数量（住宿发票明细行中的数量列）
fn extract_item_quantity(items_text: &str) -> Option<usize> {
    // 匹配 "*住宿服务*" 后面的数量，格式如: "*住宿服务*住宿费  1  420.00"
    let re = Regex::new(r"\*住宿服务\*.*?\s+(\d+)\s+[\d,.]+").ok()?;
    let caps = re.captures(items_text)?;
    caps.get(1)?.as_str().parse().ok()
}

/// 基于区域的分类（更准确）
fn classify_from_regions(
    items_text: &str,
    seller_text: &str,
    item_name: &str,
    seller_name: &str,
) -> InvoiceCategory {
    // 1. 优先匹配商品明细中的服务类型码（最可靠）
    if items_text.contains("*住宿服务*") {
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

pub fn parse_structured_invoice(
    ocr_output: &OcrStructuredOutput,
    source: InvoiceSource,
) -> Result<Invoice, String> {
    parse_structured_invoice_with_templates(ocr_output, source, None)
}

pub fn parse_structured_invoice_with_templates(
    ocr_output: &OcrStructuredOutput,
    source: InvoiceSource,
    template_manager: Option<&TemplateManager>,
) -> Result<Invoice, String> {
    if let Some(manager) = template_manager {
        if let Some(template) = manager.match_template(ocr_output) {
            if let Ok(invoice) = try_parse_with_template(ocr_output, &source, template, manager) {
                return Ok(invoice);
            }
        }
    }

    let invoice_type = InvoiceTypeDetector::detect(ocr_output);

    let amount_field = AmountExtractor::new()
        .extract(ocr_output)
        .ok_or("无法识别发票金额".to_string())?;

    let amount = amount_field
        .value
        .replace(",", "")
        .parse::<f64>()
        .map_err(|e| format!("金额解析失败: {}", e))?;

    let seller_field = SellerNameExtractor::new().extract(ocr_output);
    let item_field = ItemNameExtractor::new().extract(ocr_output);
    let date_field = DateExtractor::new().extract(ocr_output);
    let number_field = InvoiceNumberExtractor::new().extract(ocr_output);

    let category = classify_from_full_text(ocr_output, &seller_field, &item_field, &invoice_type);

    let date = date_field
        .and_then(|f| DateExtractor::new().parse_to_date(&f))
        .unwrap_or_default();

    // 提取票据出发/到达城市（仅 Train/Flight 类发票）
    let all_text: String = ocr_output
        .blocks
        .iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let (departure_city, arrival_city) = extract_ticket_cities(&all_text, &category);
    let travel_date = extract_ticket_travel_date(&all_text, &category);

    Ok(Invoice {
        id: Uuid::new_v4().to_string(),
        invoice_number: number_field.map(|f| f.value).unwrap_or_default(),
        amount,
        seller_name: seller_field.map(|f| f.value).unwrap_or_default(),
        item_name: item_field.map(|f| f.value).unwrap_or_default(),
        date,
        travel_date,
        category,
        source,
        itineraries: vec![],
        itinerary_file: None,
        remarks: String::new(),
        hotel_detail: None,
        departure_city,
        arrival_city,
        toll_travel_time: None,
    })
}

fn try_parse_with_template(
    ocr_output: &OcrStructuredOutput,
    source: &InvoiceSource,
    template: &InvoiceTemplate,
    manager: &TemplateManager,
) -> Result<Invoice, String> {
    let extracted_values = manager.extract_with_template(ocr_output, template)?;

    // 用模板的分类逻辑，而非硬编码 classify_from_full_text
    let all_text = ocr_output.blocks.iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let category = match TemplateManager::classify_by_template(template, &all_text) {
        Some(cat_str) => parse_category_from_str(&cat_str),
        None => {
            // 模板无分类配置，回退硬编码
            let invoice_type = InvoiceTypeDetector::detect(ocr_output);
            classify_from_full_text(ocr_output, &None, &None, &invoice_type)
        }
    };

    let mut amount = 0.0f64;
    let mut seller_name = String::new();
    let mut invoice_number = String::new();
    let mut date = chrono::NaiveDate::default();
    let mut item_name = String::new();

    for extracted in extracted_values {
        match extracted.field_name.as_str() {
            "amount" => {
                amount = extracted.value.replace(",", "").parse::<f64>()
                    .map_err(|e| format!("金额解析失败: {}", e))?;
            }
            "seller_name" => {
                seller_name = extracted.value;
            }
            "invoice_number" => {
                invoice_number = extracted.value;
            }
            "date" => {
                date = parse_date_from_string(&extracted.value).unwrap_or_default();
            }
            "item_name" => {
                item_name = extracted.value;
            }
            _ => {}
        }
    }

    // 提取票据出发/到达城市（仅 Train/Flight 类发票）
    let all_text: String = ocr_output
        .blocks
        .iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let (departure_city, arrival_city) = extract_ticket_cities(&all_text, &category);
    let travel_date = extract_ticket_travel_date(&all_text, &category);

    Ok(Invoice {
        id: Uuid::new_v4().to_string(),
        invoice_number,
        amount,
        seller_name,
        item_name,
        date,
        travel_date,
        category,
        source: source.clone(),
        itineraries: vec![],
        itinerary_file: None,
        remarks: String::new(),
        hotel_detail: None,
        departure_city,
        arrival_city,
        toll_travel_time: None,
    })
}

/// 将分类字符串解析为 InvoiceCategory 枚举
fn parse_category_from_str(s: &str) -> InvoiceCategory {
    match s {
        "Train" => InvoiceCategory::Train,
        "Flight" => InvoiceCategory::Flight,
        "TicketChange" => InvoiceCategory::TicketChange,
        "CityTransport" => InvoiceCategory::CityTransport,
        "Hotel" => InvoiceCategory::Hotel,
        "Meal" => InvoiceCategory::Meal,
        _ => InvoiceCategory::Other,
    }
}

fn parse_date_from_string(s: &str) -> Option<chrono::NaiveDate> {
    let re_cn = Regex::new(r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日").ok()?;
    if let Some(caps) = re_cn.captures(s) {
        let y: i32 = caps[1].parse().ok()?;
        let m: u32 = caps[2].parse().ok()?;
        let d: u32 = caps[3].parse().ok()?;
        return chrono::NaiveDate::from_ymd_opt(y, m, d);
    }

    let re_iso = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").ok()?;
    if let Some(caps) = re_iso.captures(s) {
        let y: i32 = caps[1].parse().ok()?;
        let m: u32 = caps[2].parse().ok()?;
        let d: u32 = caps[3].parse().ok()?;
        return chrono::NaiveDate::from_ymd_opt(y, m, d);
    }

    None
}

pub struct InvoiceParser {
    template_manager: Arc<RwLock<TemplateManager>>,
}

impl InvoiceParser {
    pub fn new() -> Self {
        Self {
            template_manager: Arc::new(RwLock::new(TemplateManager::new())),
        }
    }

    pub fn with_config_dir<P: Into<PathBuf>>(config_dir: P) -> Result<Self, String> {
        let manager = TemplateManager::from_config_dir(config_dir.into())?;
        Ok(Self {
            template_manager: Arc::new(RwLock::new(manager)),
        })
    }

    pub fn parse(&self, ocr_output: &OcrStructuredOutput, source: InvoiceSource) -> Result<Invoice, String> {
        let manager = self.template_manager.read().map_err(|e| e.to_string())?;
        parse_structured_invoice_with_templates(ocr_output, source, Some(&manager))
    }

    pub fn reload_templates<P: Into<PathBuf>>(&self, config_dir: P) -> Result<(), String> {
        let mut manager = self.template_manager.write().map_err(|e| e.to_string())?;
        manager.reload_from_config_dir(config_dir.into())
    }

    pub fn template_manager(&self) -> Arc<RwLock<TemplateManager>> {
        Arc::clone(&self.template_manager)
    }
}

impl Default for InvoiceParser {
    fn default() -> Self {
        Self::new()
    }
}

pub fn classify_from_full_text(
    ocr: &OcrStructuredOutput,
    seller: &Option<ExtractedField>,
    item: &Option<ExtractedField>,
    invoice_type: &InvoiceType,
) -> InvoiceCategory {
    let all_text = ocr
        .blocks
        .iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    if all_text.contains("*住宿服务*") {
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

    if contains_any(&all_text, &["酒店", "宾馆", "住宿", "招待所", "民宿"]) {
        return InvoiceCategory::Hotel;
    }
    if contains_any(&all_text, &["滴滴", "网约车", "高德", "t3", "曹操", "出租"]) {
        return InvoiceCategory::CityTransport;
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
    if contains_any(&all_text, &["退票", "改签", "保险"]) {
        return InvoiceCategory::TicketChange;
    }

    if let Some(seller_field) = seller {
        let seller_lower = seller_field.value.to_lowercase();
        if contains_any(&seller_lower, &["酒店", "宾馆", "住宿"]) {
            return InvoiceCategory::Hotel;
        }
        if contains_any(&seller_lower, &["滴滴", "高德", "网约车"]) {
            return InvoiceCategory::CityTransport;
        }
    }

    if let Some(item_field) = item {
        let item_lower = item_field.value.to_lowercase();
        if contains_any(&item_lower, &["住宿", "房费"]) {
            return InvoiceCategory::Hotel;
        }
        if contains_any(&item_lower, &["交通", "打车", "出行"]) {
            return InvoiceCategory::CityTransport;
        }
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
    let re_step0 = Regex::new(r"([\d,]+\.\d{2})\s*价税合计").map_err(|e| e.to_string())?;
    if let Some(caps) = re_step0.captures(text) {
        let amount_str = caps[1].replace(",", "");
        return amount_str.parse::<f64>().map_err(|e| e.to_string());
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
    let buyer_keywords = ["购买方", "国防", "大学", "学院", "医院"];
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
        if !buyer_keywords.iter().any(|kw| candidate.contains(kw)) {
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

fn extract_seller_by_coords(texts: &[OcrTextItem]) -> String {
    let re = Regex::new(r"名称[：:]\s*(\S.+)").unwrap();
    let mut best_x = 0.0f64;
    let mut best_name = String::new();
    for item in texts {
        if let Some(caps) = re.captures(&item.text) {
            let name = caps[1].trim();
            if name.len() <= 2 { continue; }
            if let Some(coords) = &item.box_coords {
                if let Some(x) = coords.get("points").and_then(|p| p.get(0)).and_then(|p| p.get("x")).and_then(|v| v.as_f64()) {
                    if x > best_x {
                        best_x = x;
                        best_name = name.to_string();
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
    } else if contains_any(&combined_lower, &["航空", "机票", "机场", "航班"]) {
        InvoiceCategory::Flight
    } else if contains_any(&combined_lower, &["退票", "改签", "保险"]) {
        InvoiceCategory::TicketChange
    } else if contains_any(
        &combined_lower,
        &["出租", "网约车", "滴滴", "高德", "t3", "曹操"],
    ) {
        InvoiceCategory::CityTransport
    } else if contains_any(&combined_lower, &["酒店", "宾馆", "住宿", "招待所", "民宿"]) {
        InvoiceCategory::Hotel
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
    fn test_parse_structured_invoice_full() {
        let ocr = create_ocr_output(vec![
            "销售方信息",
            "名称：四川景澜酒店管理有限公司",
            "价税合计：¥1045.24",
            "项目名称：住宿服务",
            "开票日期：2025年06月15日",
        ]);

        let result = parse_structured_invoice(
            &ocr,
            InvoiceSource::Pdf("test.pdf".to_string()),
        );
        assert!(result.is_ok());
        let invoice = result.unwrap();
        assert!((invoice.amount - 1045.24).abs() < 0.01);
        assert_eq!(invoice.seller_name, "四川景澜酒店管理有限公司");
        assert_eq!(invoice.category, InvoiceCategory::Hotel);
    }

    #[test]
    fn test_classify_from_full_text_with_tax_code() {
        let ocr = create_ocr_output(vec!["*住宿服务*", "金额：500.00"]);
        let result = classify_from_full_text(&ocr, &None, &None, &InvoiceType::Other);
        assert_eq!(result, InvoiceCategory::Hotel);
    }

    #[test]
    fn test_classify_from_full_text_flight() {
        let ocr = create_ocr_output(vec!["机票行程单", "航班号：CA1234"]);
        let result = classify_from_full_text(&ocr, &None, &None, &InvoiceType::FlightInvoice);
        assert_eq!(result, InvoiceCategory::Flight);
    }

    #[test]
    fn test_classify_from_full_text_city_transport() {
        let ocr = create_ocr_output(vec!["滴滴出行电子发票", "网约车服务"]);
        let result =
            classify_from_full_text(&ocr, &None, &None, &InvoiceType::RideHailingInvoice);
        assert_eq!(result, InvoiceCategory::CityTransport);
    }

    #[test]
    fn test_classify_from_full_text_keywords() {
        let ocr = create_ocr_output(vec!["如家酒店住宿费", "金额：300.00"]);
        let result = classify_from_full_text(&ocr, &None, &None, &InvoiceType::Other);
        assert_eq!(result, InvoiceCategory::Hotel);
    }

    #[test]
    fn test_parse_structured_invoice_with_no_amount() {
        let ocr = create_ocr_output(vec!["发票号码：12345678"]);
        let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("test.pdf".to_string()));
        assert!(result.is_err());
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
    fn test_classify_from_full_text_with_seller_field() {
        let ocr = create_ocr_output(vec!["其他发票", "金额：100.00"]);
        let seller = Some(ExtractedField {
            value: "如家酒店".to_string(),
            confidence: 0.9,
            strategy: "test".to_string(),
            source_position: None,
        });
        let result = classify_from_full_text(&ocr, &seller, &None, &InvoiceType::Other);
        assert_eq!(result, InvoiceCategory::Hotel);
    }

    #[test]
    fn test_classify_from_full_text_train_with_transport_service_tax_code() {
        // 火车票增值税发票使用 *运输服务* 税收编码，不应误识别为 CityTransport
        let ocr = create_ocr_output(vec!["*运输服务*", "中国铁路", "高铁", "金额：200.00"]);
        let result = classify_from_full_text(&ocr, &None, &None, &InvoiceType::TrainInvoice);
        assert_eq!(result, InvoiceCategory::Train);
    }

    #[test]
    fn test_classify_from_full_text_train_with_passenger_tax_code() {
        // 火车票使用 *客运服务* 税收编码
        let ocr = create_ocr_output(vec!["*客运服务*", "铁路", "金额：150.00"]);
        let result = classify_from_full_text(&ocr, &None, &None, &InvoiceType::TrainInvoice);
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
    fn test_classify_from_full_text_with_item_field() {
        let ocr = create_ocr_output(vec!["其他发票", "金额：100.00"]);
        let item = Some(ExtractedField {
            value: "交通服务费".to_string(),
            confidence: 0.9,
            strategy: "test".to_string(),
            source_position: None,
        });
        let result = classify_from_full_text(&ocr, &None, &item, &InvoiceType::Other);
        assert_eq!(result, InvoiceCategory::CityTransport);
    }

    #[test]
    fn test_invoice_parser_new() {
        let parser = InvoiceParser::new();
        let ocr = create_ocr_output(vec!["测试发票", "价税合计：¥500.00"]);
        let result = parser.parse(&ocr, InvoiceSource::Pdf("test.pdf".to_string()));
        assert!(result.is_ok());
        let invoice = result.unwrap();
        assert!((invoice.amount - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_invoice_parser_with_templates() {
        use crate::parser::template_manager::{FieldDefinition, FieldStrategy, InvoiceTemplate};
        
        let parser = InvoiceParser::new();
        
        let template = InvoiceTemplate {
            template_id: "test_template".to_string(),
            name: "测试模板".to_string(),
            enabled: true,
            priority: 0,
            keywords: vec!["测试发票".to_string()],
            category: None,
            category_keywords: None,
            fields: vec![
                FieldDefinition {
                    name: "amount".to_string(),
                    required: true,
                    strategies: vec![FieldStrategy {
                        strategy_type: "regex".to_string(),
                        pattern: Some("价税合计[：:￥¥]*\\s*([\\d,]+\\.?\\d*)".to_string()),
                        section_keyword: None,
                        field_keyword: None,
                        confidence: 0.9,
                    }],
                },
            ],
        };

        let tm = parser.template_manager();
        let mut manager = tm.write().unwrap();
        manager.add_template(template);

        let ocr = create_ocr_output(vec!["测试发票", "价税合计：¥123.45"]);
        let result = parser.parse(&ocr, InvoiceSource::Pdf("test.pdf".to_string()));
        assert!(result.is_ok());
        let invoice = result.unwrap();
        assert!((invoice.amount - 123.45).abs() < 0.01);
    }

    #[test]
    fn test_parse_date_from_string() {
        let date1 = parse_date_from_string("2025年08月15日");
        assert!(date1.is_some());
        assert_eq!(date1.unwrap(), chrono::NaiveDate::from_ymd_opt(2025, 8, 15).unwrap());

        let date2 = parse_date_from_string("2025-08-15");
        assert!(date2.is_some());
        assert_eq!(date2.unwrap(), chrono::NaiveDate::from_ymd_opt(2025, 8, 15).unwrap());

        let date3 = parse_date_from_string("invalid");
        assert!(date3.is_none());
    }

    #[test]
    fn test_backward_compatibility_with_template_manager() {
        let parser = InvoiceParser::new();
        let ocr = create_ocr_output(vec![
            "销售方信息",
            "名称：四川景澜酒店管理有限公司",
            "价税合计：¥1045.24",
            "项目名称：住宿服务",
        ]);

        let result = parser.parse(&ocr, InvoiceSource::Pdf("test.pdf".to_string()));
        assert!(result.is_ok());
        let invoice = result.unwrap();
        assert!((invoice.amount - 1045.24).abs() < 0.01);
        assert_eq!(invoice.category, InvoiceCategory::Hotel);
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
    fn test_template_classification_overrides_hardcoded() {
        use std::collections::HashMap;
        use crate::parser::template_manager::{FieldDefinition, FieldStrategy};

        let blocks = vec![OcrTextBlock {
            text: "增值税普通发票 价税合计：¥100.00 名称：测试餐饮店".to_string(),
            confidence: 0.95,
            bbox: BoundingBox { x: 0.0, y: 0.0, width: 200.0, height: 20.0 },
            line_index: 0,
            block_type: TextBlockType::KeyValue,
        }];
        let ocr = OcrStructuredOutput { blocks, layout: PageLayout::default() };

        // 模板带 category_keywords，应返回模板分类
        let template = InvoiceTemplate {
            template_id: "test_cat".to_string(),
            name: "测试分类".to_string(),
            enabled: true,
            priority: 100,
            keywords: vec!["增值税普通发票".to_string()],
            category: Some("Other".to_string()),
            category_keywords: Some(HashMap::from([
                ("Meal".to_string(), vec!["餐饮".to_string()]),
            ])),
            fields: vec![FieldDefinition {
                name: "amount".to_string(),
                required: true,
                strategies: vec![FieldStrategy {
                    strategy_type: "regex".to_string(),
                    pattern: Some("价税合计[：:￥¥]*\\s*([\\d,]+\\.?\\d*)".to_string()),
                    section_keyword: None,
                    field_keyword: None,
                    confidence: 0.9,
                }],
            }],
        };

        let mut manager = TemplateManager::new();
        manager.add_template(template);

        let invoice = parse_structured_invoice_with_templates(
            &ocr,
            InvoiceSource::Pdf("test.pdf".to_string()),
            Some(&manager),
        ).unwrap();

        assert_eq!(invoice.category, InvoiceCategory::Meal);
        assert!((invoice.amount - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_fallback_to_hardcoded_when_no_template_matches() {
        let blocks = vec![OcrTextBlock {
            text: "某未知格式发票 金额：¥200.00".to_string(),
            confidence: 0.95,
            bbox: BoundingBox { x: 0.0, y: 0.0, width: 200.0, height: 20.0 },
            line_index: 0,
            block_type: TextBlockType::KeyValue,
        }];
        let ocr = OcrStructuredOutput { blocks, layout: PageLayout::default() };

        // 空模板管理器，无模板匹配
        let manager = TemplateManager::new();
        let invoice = parse_structured_invoice_with_templates(
            &ocr,
            InvoiceSource::Pdf("test.pdf".to_string()),
            Some(&manager),
        ).unwrap();

        // 应回退到硬编码逻辑
        assert!((invoice.amount - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_regression_template_vs_hardcoded_same_result() {
        use std::collections::HashMap;
        use crate::parser::template_manager::{FieldDefinition, FieldStrategy};

        // 模拟一张增值税普通发票的 OCR 文本
        let blocks = vec![
            OcrTextBlock {
                text: "增值税普通发票".to_string(),
                confidence: 0.95,
                bbox: BoundingBox { x: 0.0, y: 0.0, width: 200.0, height: 20.0 },
                line_index: 0,
                block_type: TextBlockType::Other,
            },
            OcrTextBlock {
                text: "价税合计：¥1,234.56".to_string(),
                confidence: 0.95,
                bbox: BoundingBox { x: 0.0, y: 100.0, width: 200.0, height: 20.0 },
                line_index: 5,
                block_type: TextBlockType::KeyValue,
            },
            OcrTextBlock {
                text: "名称：测试酒店".to_string(),
                confidence: 0.95,
                bbox: BoundingBox { x: 0.0, y: 150.0, width: 200.0, height: 20.0 },
                line_index: 7,
                block_type: TextBlockType::KeyValue,
            },
            OcrTextBlock {
                text: "2024年05月20日".to_string(),
                confidence: 0.95,
                bbox: BoundingBox { x: 0.0, y: 200.0, width: 200.0, height: 20.0 },
                line_index: 9,
                block_type: TextBlockType::KeyValue,
            },
        ];
        let ocr = OcrStructuredOutput { blocks, layout: PageLayout::default() };
        let source = InvoiceSource::Pdf("test.pdf".to_string());

        // 无模板：走硬编码
        let hardcoded = parse_structured_invoice_with_templates(&ocr, source.clone(), None).unwrap();

        // 有模板（等价正则）：走模板
        let template = InvoiceTemplate {
            template_id: "regression_test".to_string(),
            name: "回归测试".to_string(),
            enabled: true,
            priority: 10,
            keywords: vec!["增值税普通发票".to_string()],
            category: Some("Other".to_string()),
            category_keywords: Some(HashMap::from([
                ("Hotel".to_string(), vec!["酒店".to_string()]),
            ])),
            fields: vec![
                FieldDefinition {
                    name: "amount".to_string(),
                    required: true,
                    strategies: vec![FieldStrategy {
                        strategy_type: "regex".to_string(),
                        pattern: Some("价税合计[：:￥¥]*\\s*([\\d,]+\\.?\\d*)".to_string()),
                        section_keyword: None,
                        field_keyword: None,
                        confidence: 0.9,
                    }],
                },
                FieldDefinition {
                    name: "seller_name".to_string(),
                    required: false,
                    strategies: vec![FieldStrategy {
                        strategy_type: "regex".to_string(),
                        pattern: Some("名称[：:]\\s*(\\S+)".to_string()),
                        section_keyword: None,
                        field_keyword: None,
                        confidence: 0.85,
                    }],
                },
            ],
        };
        let mut manager = TemplateManager::new();
        manager.add_template(template);
        let templated = parse_structured_invoice_with_templates(&ocr, source, Some(&manager)).unwrap();

        // 金额应一致
        assert!((hardcoded.amount - templated.amount).abs() < 0.001,
            "金额不一致: 硬编码={} 模板={}", hardcoded.amount, templated.amount);
        assert!((hardcoded.amount - 1234.56).abs() < 0.001);

        // 销售方应一致
        assert!(!templated.seller_name.is_empty(), "模板模式销售方不应为空");
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
}
