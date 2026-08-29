use crate::models::invoice::{Invoice, InvoiceCategory, InvoiceSource, Itinerary};
use crate::ocr::OcrEngine;
#[cfg(feature = "pdfplumber")]
use crate::parser::cell_extractor;
use crate::parser::dedup::deduplicate_invoices;
use crate::parser::invoice_parser::{
    extract_hotel_statement_detail, extract_ticket_cities, extract_ticket_travel_date,
    extract_toll_travel_time, parse_hotel_detail, parse_invoice_text, parse_nights_from_remarks,
};
#[cfg(feature = "pdfplumber")]
use crate::parser::itinerary_parser::parse_itinerary_from_tables;
use crate::parser::itinerary_parser::{
    compute_incomplete_fields, cross_validate_amounts, cross_validate_with_printed_total,
    enrich_itinerary_years, extract_itinerary_printed_total, has_incomplete_entries,
    parse_itinerary_text, parse_itinerary_with_coords_pages_and_fallback,
};
use crate::pdf::text_extractor::{self, classify_pdf_document_type, PdfDocumentType};
use std::path::{Path, PathBuf};

/// 提取策略配置（保留兼容性，当前发票解析仅使用单元格提取）
#[derive(Debug, Clone)]
pub struct ExtractionConfig {
    pub enable_word_fallback: bool,
    pub enable_text_fallback: bool,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            enable_word_fallback: false,
            enable_text_fallback: false,
        }
    }
}

#[allow(dead_code)]
fn is_likely_garbled_seller(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.chars().count() < 3 {
        return true;
    }
    if trimmed.contains("名称：") || trimmed.contains("名称:") {
        return true;
    }
    if trimmed
        .chars()
        .all(|c| c.is_whitespace() || "名称：:，,。.、；;（）()".contains(c))
    {
        return true;
    }
    if trimmed.contains('<') || trimmed.contains('>') {
        return true;
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return true;
    }
    let label_chars = "购买售销方密";
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words
        .iter()
        .any(|w| w.chars().count() == 1 && w.chars().all(|c| label_chars.contains(c)))
    {
        return true;
    }
    false
}

/// 从行程单解析出的行程明细集合
#[derive(Debug, Clone, serde::Serialize)]
pub struct ItineraryDoc {
    pub file_name: String,
    pub itineraries: Vec<Itinerary>,
    pub total_amount: f64,
    /// 行程单上印制的"合计金额"（精确值，用于匹配发票）
    /// None 表示未能从行程单中提取到合计金额（需回退容差匹配）
    pub printed_total: Option<f64>,
}

/// 解析目录结果
#[derive(Debug, serde::Serialize)]
pub struct ParseResult {
    pub invoices: Vec<Invoice>,
    pub errors: Vec<(String, String)>,
    /// 批次内去重命中的重复发票号列表
    pub duplicates: Vec<String>,
}

/// 解析单个发票 PDF：用 pdfplumber 提取表格单元格 + 原始 Word 文本。
/// 全部字段由单元格匹配提取，不依赖文本正则解析。
pub fn parse_invoice_from_pdf(
    pdf_path: &str,
    engine: &mut OcrEngine,
    _config: &ExtractionConfig,
) -> Result<Invoice, String> {
    let source = InvoiceSource::Pdf(pdf_path.to_string());

    #[cfg(feature = "pdfplumber")]
    let (raw_text, tables, raw_words, flat_texts) = {
        match text_extractor::extract_pdf_column_aware(pdf_path) {
            Ok(extraction) => {
                if extraction.tables.iter().all(|p| p.is_empty()) {
                    eprintln!("  [pdfplumber] 无表格结构，回退到 OCR");
                    let ocr_items = extract_ocr_text_only(pdf_path, engine)?;
                    return check_and_parse(ocr_items, source);
                }
                // word 级 OcrTextItem（未合并，保留原始 Word 边界与坐标），
                // 供 cell 失败时回退到文本解析使用——列感知合并会拆散日期/发票号等连续 token，
                // 用 word 级保证 parse_invoice_text 看到与 pdfplumber Word 一致的文本
                let flat_texts: Vec<crate::ocr::OcrTextItem> = extraction
                    .word_pages
                    .iter()
                    .flat_map(|p| p.texts.clone())
                    .collect();
                let text: String = extraction
                    .raw_words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join("");
                // CID 字体乱码检测：文字型 PDF 但 CID 映射失败时文字为韩文/PUA/替换符
                if text_extractor::is_garbled_text(&text, 0.3) {
                    eprintln!("  [pdfplumber] 文字乱码（CID 字体映射失败），回退到 OCR");
                    let ocr_items = extract_ocr_text_only(pdf_path, engine)?;
                    return check_and_parse(ocr_items, source);
                }
                eprintln!(
                    "  [pdfplumber] {} 个原始Word, {} 页表格",
                    extraction.raw_words.len(),
                    extraction.tables.len()
                );
                (text, extraction.tables, extraction.raw_words, flat_texts)
            }
            Err(e) => {
                eprintln!("  [pdfplumber] 失败: {}，回退到 OCR", e);
                let ocr_items = extract_ocr_text_only(pdf_path, engine)?;
                return check_and_parse(ocr_items, source);
            }
        }
    };
    #[cfg(not(feature = "pdfplumber"))]
    {
        let ocr_items = extract_ocr_text_only(pdf_path, engine)?;
        return check_and_parse(ocr_items, source);
    }

    // 文档类型检查 — 行程单/结账单不应走发票解析
    if raw_text.contains("行程单")
        || raw_text.contains("行程报销单")
        || raw_text.contains("电子行程单")
    {
        return Err("非发票类型: Itinerary".to_string());
    }
    if raw_text.contains("结账单") {
        return Err("非发票类型: Bill".to_string());
    }

    // 单元格提取建票
    let cell_fields = cell_extractor::extract_fields_from_tables(&tables);

    // 对商品详情提取 item_detail（商家自定义名称）
    // 优先用单元格的按列聚合文本（Type 4，跨行不丢字），回退到 raw_words 重新合并
    let item_detail = extract_item_detail(
        cell_fields.item_cell_text.as_deref(),
        cell_fields.item_cell_bbox,
        &raw_words,
    );

    if cell_fields.seller_name.is_none() || cell_fields.amount.unwrap_or(0.0) <= 0.0 {
        // cell 路径未提取到 seller 或 amount 时回退到文本解析：
        // - 火车票等无表格框线票据：pdfplumber 误检测出伪表格
        // - cell 提取逻辑未能覆盖的特殊布局
        // （parse_invoice_text 已为火车票定制 extract_ticket_cities 等）。
        eprintln!("  [cell] 单元格未提取到有效数据（无表格框线？），回退到 Word 框文本解析");
        let mut inv = parse_invoice_text(&flat_texts, source)?;
        // pdfplumber 将日期拆散为多个 word，正则无法跨 word 匹配。
        // 用 char 级间距过滤重建行文本，覆盖 travel_date（仅 Train/Flight）。
        if inv.category == crate::models::invoice::InvoiceCategory::Train
            || inv.category == crate::models::invoice::InvoiceCategory::Flight
        {
            let lines = text_extractor::reconstruct_lines_from_chars(&raw_words);
            if !lines.is_empty() {
                let joined = lines.join("\n");
                if let Some((td, tt)) =
                    crate::parser::invoice_parser::extract_ticket_travel_date(&joined, &inv.category)
                {
                    eprintln!("  [cell] char-level reconstruct → travel_date={td:?} travel_time={tt:?}");
                    inv.travel_date = td;
                    if tt.is_some() {
                        inv.travel_time = tt;
                    }
                }
            }
        }
        return Ok(inv);
    }
    build_invoice_from_cells(cell_fields, &item_detail, &raw_text, source.clone())
}

/// 从单元格提取结果 + 文本头部信息（发票号、日期）构建 Invoice，完全跳过文本正则解析。
#[cfg(feature = "pdfplumber")]
fn build_invoice_from_cells(
    fields: cell_extractor::CellInvoiceFields,
    item_detail: &Option<String>,
    all_text: &str,
    source: InvoiceSource,
) -> Result<Invoice, String> {
    use crate::models::invoice::HotelDetail;

    // 发票号（8位老式号码~30位全电发票号码）。注意：发票号/日期不在表格内，
    // 不能走单元格提取，只能全文正则（见 CLAUDE.md 智能体定义）
    let invoice_number = {
        let re = regex::Regex::new(r"发票号码[：:\s]*(\d{8,30})").ok();
        re.and_then(|r| r.captures(all_text))
            .map(|c| c[1].to_string())
            .or_else(|| {
                let re_num = regex::Regex::new(r"(\d{18,22})").ok()?;
                re_num.captures(all_text).map(|c| c[1].to_string())
            })
            .unwrap_or_default()
    };
    // 日期：匹配 YYYY年MM月DD日 / YYYY-MM-DD / YYYY/MM/DD（日 可选）
    // 用 captures_iter 遍历所有匹配，跳过非日期数字串（税号、发票号等可能先匹配但范围检查失败）
    let date = {
        let re =
            regex::Regex::new(r"(\d{4})[\s]*[年\-/][\s]*(\d{1,2})[\s]*[月\-/][\s]*(\d{1,2})").ok();
        re.and_then(|r| {
            r.captures_iter(all_text).find_map(|c| {
                let y: i32 = c[1].parse().ok()?;
                let m: u32 = c[2].parse().ok()?;
                let d: u32 = c[3].parse().ok()?;
                if y < 2000 || m > 12 || d > 31 {
                    return None;
                }
                chrono::NaiveDate::from_ymd_opt(y, m, d)
            })
        })
        .unwrap_or_default()
    };

    let seller = fields.seller_name.unwrap_or_default();
    let amount = fields.amount.unwrap_or(0.0);
    let item_name = fields.item_name.unwrap_or_default();
    let item_detail = item_detail.as_deref().unwrap_or_default();
    let remarks = fields.remarks.unwrap_or_default();

    // 类别：优先用 item_detail（商家自定义名称如"住宿费"、"代订机票"），其次用 item_name 税收编码简称
    let category = classify_from_item(&item_name, &item_detail, &seller, all_text, &remarks);

    // 住宿发票：从备注 / 全文提取入住/离店日期
    let hotel_detail = if category == InvoiceCategory::Hotel {
        let detail = parse_hotel_detail(&remarks, date)
            .or_else(|| extract_hotel_statement_detail(all_text, date));
        if let Some(mut d) = detail {
            if d.nights == 0 {
                if let Some(n) = parse_nights_from_remarks(&remarks) {
                    d.nights = n;
                }
            }
            // parse_hotel_detail 返回的 nightly_rate=0，这里用 amount/nights 覆盖
            d.nightly_rate = amount / d.nights.max(1) as f64;
            Some(d)
        } else {
            // 最低回退：至少设置 nightly_rate
            let nights = parse_nights_from_remarks(&remarks).unwrap_or(1).max(1);
            Some(HotelDetail {
                check_in: None,
                check_out: None,
                nights,
                nightly_rate: amount / nights as f64,
            })
        }
    } else {
        None
    };

    // 票据城市/日期（仅 Train/Flight）
    // 优先从单元格提取的备注（精确）提取，回退到全文（备注为空或未匹配时）
    let (departure_city, arrival_city) = {
        let (d, a) = extract_ticket_cities(&remarks, &category);
        if d.is_some() && a.is_some() {
            (d, a)
        } else {
            let (d2, a2) = extract_ticket_cities(all_text, &category);
            (d.or(d2), a.or(a2))
        }
    };
    let (travel_date, travel_time) = extract_ticket_travel_date(&remarks, &category)
        .or_else(|| extract_ticket_travel_date(all_text, &category))
        .unwrap_or((None, None));

    // 通行费发票：从备注提取通行时间
    let toll_travel_time = if category == InvoiceCategory::Toll {
        extract_toll_travel_time(&remarks)
    } else {
        None
    };

    eprintln!("  [cell] invoice built: seller={seller}, amount={amount}, item={item_name}, no={invoice_number}, date={date}, cat={category:?}");

    Ok(Invoice {
        id: uuid::Uuid::new_v4().to_string(),
        invoice_number,
        seller_name: seller,
        amount,
        date,
        item_name,
        remarks,
        hotel_detail,
        category,
        source,
        itineraries: Vec::new(),
        itinerary_file: None,
        departure_city,
        arrival_city,
        travel_date,
        travel_time,
        toll_travel_time,
    })
}

/// 从商品详情文本提取 item_detail（商家自定义名称）。
/// 优先用单元格按列聚合文本（Type 4，跨行不丢字如"组合险"换行到第二行），
/// 回退到 raw_words 重新做列感知合并。
///
/// 提取逻辑：定位 `*税收编码*` 后的内容，跳过 `**` 规格分隔符，
/// 收集 CJK 字符（允许中间空格——列聚合时同列跨行字会被空格分隔），
/// 遇到数字/¥/表头词（项目名称/规格型号/单位/数量/单价/金额/税率/税额/合计）停止。
#[cfg(feature = "pdfplumber")]
fn extract_item_detail(
    cell_text: Option<&str>,
    bbox: Option<(f64, f64, f64, f64)>,
    raw_words: &[pdfplumber::Word],
) -> Option<String> {
    // 候选文本：优先单元格按列聚合文本，回退到 raw_words 重新合并
    let fallback_merged;
    let text = match cell_text {
        Some(t) if !t.is_empty() => t,
        _ => {
            let (x0, top, x1, bottom) = bbox?;
            fallback_merged =
                text_extractor::column_aware_merge_in_bbox(raw_words, x0, top, x1, bottom);
            if fallback_merged.is_empty() {
                return None;
            }
            &fallback_merged
        }
    };

    // 定位 *税收编码* 后的内容
    let pos = text.find('*')?;
    let end = text[pos + 1..].find('*')?;
    let after = &text[pos + end + 2..];

    // 商品名在 *编码* 和 **（规格分隔符）之间，或 *编码* 和数字/表头之间。
    // 先截到第二个 **（如果有），否则截到数字/表头词。
    // ponytail: ** 是税收编码后的规格分隔符，商品名不会含 **，直接截断最稳
    let segment = match after.find("**") {
        Some(p) => after[..p].trim(),
        None => after.trim_start(),
    };

    // 表头词：遇到即停（列聚合可能把表头混入商品名后）
    const STOP_WORDS: &[&str] = &[
        "项目名称",
        "规格型号",
        "单位",
        "数量",
        "单价",
        "金额",
        "税率",
        "税额",
        "合计",
    ];
    // 单字停止词：列聚合时"合计"被拆到不同列只剩"合"，但"组合险"含"合"不能误伤
    // → "合"后跟"计"或空格/结尾才停，跟其他字（如"险"）则继续
    fn is_standalone_he(chars: &[char], i: usize) -> bool {
        // 当前是"合"，看下一个非空格字符
        let next = chars.get(i + 1).copied();
        match next {
            None => true,                         // "合"在末尾
            Some(c) if c.is_whitespace() => true, // "合 " 后面是别的列
            Some('计') => true,                   // "合计"
            _ => false,                           // "组合险"等 → 继续
        }
    }
    // 计量单位单字：商品名后紧跟单位时停（"国内机票份"→"国内机票"）
    const UNIT_CHARS: &[char] = &[
        '份', '个', '张', '次', '台', '套', '件', '升', '吨', '度', '人', '间', '晚', '日', '天',
        '月', '年', '页', '本', '册', '盒', '箱', '包', '批',
    ];

    let mut detail = String::new();
    let chars: Vec<char> = segment.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // 数字或 ¥ → 停
        if c.is_ascii_digit() || c == '¥' {
            break;
        }
        // "合" 单独判断（避免误伤"组合险"）
        if c == '合' && is_standalone_he(&chars, i) {
            break;
        }
        // 计量单位单字 → 停（商品名后紧跟单位）
        if UNIT_CHARS.contains(&c) {
            break;
        }
        // 空格 → 跳过（列聚合的同列跨行分隔），但先检查后面是否表头词
        if c.is_whitespace() {
            let rest: String = chars[i..].iter().take(8).collect();
            if STOP_WORDS.iter().any(|w| rest.starts_with(w)) {
                break;
            }
            i += 1;
            continue;
        }
        // 检查当前位置是否是表头词开头
        let rest: String = chars[i..].iter().take(8).collect();
        if STOP_WORDS.iter().any(|w| rest.starts_with(w)) {
            break;
        }
        // CJK 或允许的字符 → 收集
        detail.push(c);
        i += 1;
    }

    let detail = detail.trim().to_string();
    if detail.is_empty() {
        None
    } else {
        Some(detail)
    }
}

/// 分类：优先用 item_detail（商家自定义名称，精确），其次 item_name（税收编码简称），回退到上下文。
#[cfg(feature = "pdfplumber")]
fn classify_from_item(
    item_name: &str,
    item_detail: &str,
    seller_name: &str,
    all_text: &str,
    remarks: &str,
) -> InvoiceCategory {
    // 保险优先：税收编码简称（item_name）含"保险"时直接归 Insurance，
    // 防止 item_detail（商家自定义名如"境内机票航意航延"）含"机票"误判为 Flight
    if item_name.contains("保险") {
        return InvoiceCategory::Insurance;
    }

    // ── 商家自定义名称判断（优先级最高）──
    if !item_detail.is_empty() {
        let d = item_detail;
        // 退改签优先识别（"退票费"含"票"但不是机票，"经纪代理"后面会误判为 Flight）
        if d.contains("退票") || d.contains("改签") {
            return InvoiceCategory::TicketChange;
        }
        // 保险优先级最高（"航空意外险"含"航空"但不该判为机票）
        // "航意"=航意险、"航延"=航延险，均为保险专有缩写，不会出现在真实机票上
        if d.contains("保险")
            || d.contains("意外险")
            || d.contains("意外")
            || d.contains("航意")
            || d.contains("航延")
        {
            return InvoiceCategory::Insurance;
        }
        if d.contains("住宿") || d.contains("酒店") || d.contains("宾馆") {
            return InvoiceCategory::Hotel;
        }
        if d.contains("机票") || d.contains("航空") || d.contains("客票") {
            return InvoiceCategory::Flight;
        }
        if d.contains("火车") || d.contains("车票") || d.contains("铁路") {
            return InvoiceCategory::Train;
        }
        if d.contains("餐饮") || d.contains("餐费") || d.contains("餐饮费") {
            return InvoiceCategory::Meal;
        }
        if d.contains("通行费") || d.contains("高速") {
            return InvoiceCategory::Toll;
        }
        if d.contains("客运") || d.contains("运输") {
            return InvoiceCategory::CityTransport;
        }
        // 预付卡：需结合 seller 判断（天府通→市内交通，否则 Other）
        if d.contains("预付卡") {
            if seller_name.contains("天府通")
                || seller_name.contains("轨道交通")
                || seller_name.contains("公交")
                || seller_name.contains("地铁")
            {
                return InvoiceCategory::CityTransport;
            }
            return InvoiceCategory::Other;
        }
    }

    // ── 税收编码简称 ──
    // 退改签兜底（item_detail 为空时，从 all_text/remarks 检查）
    if all_text.contains("退票")
        || all_text.contains("改签")
        || remarks.contains("退票")
        || remarks.contains("改签")
    {
        return InvoiceCategory::TicketChange;
    }
    if item_name.contains("住宿") {
        return InvoiceCategory::Hotel;
    }
    if item_name.contains("运输服务") || item_name.contains("客运服务") {
        if all_text.contains("火车") || all_text.contains("车次") || all_text.contains("铁路")
        {
            return InvoiceCategory::Train;
        }
        if all_text.contains("航班") || all_text.contains("机票") {
            return InvoiceCategory::Flight;
        }
        return InvoiceCategory::CityTransport;
    }
    if item_name.contains("经纪代理")
        || item_name.contains("航空运输")
        || item_name.contains("旅客运输")
    {
        return InvoiceCategory::Flight;
    }
    if item_name.contains("保险") {
        return InvoiceCategory::Insurance;
    }
    if item_name.contains("餐饮") {
        return InvoiceCategory::Meal;
    }

    // ── 生产生活服务（2026年新编码，囊括所有服务类）──
    if item_name.contains("生产生活服务") {
        // 商家自定义名称已在上面处理过；回退到上下文
        let context: String = format!("{remarks} {seller_name} {all_text}");
        if seller_name.contains("酒店")
            || seller_name.contains("宾馆")
            || context.contains("住宿费")
            || context.contains("住宿服务")
        {
            return InvoiceCategory::Hotel;
        }
        if context.contains("高速") || context.contains("通行费") || context.contains("收费车道")
        {
            return InvoiceCategory::Toll;
        }
        if remarks.contains("经济舱")
            || remarks.contains("头等舱")
            || remarks.contains("商务舱")
            || all_text.contains("航班")
            || all_text.contains("机票")
            || seller_name.contains("航空服务")
        {
            return InvoiceCategory::Flight;
        }
    }

    // ── seller 关键词回退 ──
    if seller_name.contains("滴滴")
        || seller_name.contains("出行科技")
        || seller_name.contains("优行")
        || seller_name.contains("天府通")
        || seller_name.contains("轨道交通")
        || seller_name.contains("公交")
        || seller_name.contains("地铁")
    {
        return InvoiceCategory::CityTransport;
    }
    if seller_name.contains("酒店") || seller_name.contains("宾馆") || seller_name.contains("民宿")
    {
        return InvoiceCategory::Hotel;
    }
    if seller_name.contains("高速") || seller_name.contains("公路") {
        return InvoiceCategory::Toll;
    }
    InvoiceCategory::Other
}

fn extract_ocr_text_only(
    pdf_path: &str,
    engine: &mut OcrEngine,
) -> Result<Vec<crate::ocr::OcrTextItem>, String> {
    if !engine.health().unwrap_or(false) {
        return Err("OCR 模型未安装".to_string());
    }
    let resp = engine.recognize_pdf(pdf_path)?;
    Ok(resp.pages.iter().flat_map(|p| p.texts.clone()).collect())
}

fn extract_ocr_text(
    pdf_path: &str,
    engine: &mut OcrEngine,
) -> Result<Vec<crate::ocr::OcrPageResult>, String> {
    let resp = engine.recognize_pdf(pdf_path)?;
    Ok(resp.pages)
}

/// 带坐标的文字提取：优先使用 pdfplumber（feature-gated），回退到 OCR
#[cfg(feature = "pdfplumber")]
pub fn extract_text_with_coords_or_fallback(
    pdf_path: &str,
    engine: &mut OcrEngine,
) -> Result<Vec<crate::ocr::OcrTextItem>, String> {
    match text_extractor::extract_text_with_coords_flat(pdf_path) {
        Ok(items)
            if text_extractor::has_sufficient_text(&items, 20)
                && !text_extractor::is_garbled_items(&items, 0.3) =>
        {
            eprintln!("  [pdfplumber] 提取到 {} 个带坐标文本项", items.len());
            Ok(items)
        }
        Ok(items) if text_extractor::has_sufficient_text(&items, 20) => {
            eprintln!("  [pdfplumber] 文字乱码（CID 字体映射失败），回退到 OCR");
            extract_ocr_text_only(pdf_path, engine)
        }
        _ => {
            eprintln!("  [pdfplumber] 不可用或无文本，回退到 OCR");
            extract_ocr_text_only(pdf_path, engine)
        }
    }
}

#[cfg(not(feature = "pdfplumber"))]
pub fn extract_text_with_coords_or_fallback(
    pdf_path: &str,
    engine: &mut OcrEngine,
) -> Result<Vec<crate::ocr::OcrTextItem>, String> {
    extract_ocr_text_only(pdf_path, engine)
}

/// 解析单个发票图片：OCR 识别后分类检查
pub fn parse_invoice_from_image(
    image_path: &str,
    engine: &mut OcrEngine,
) -> Result<Invoice, String> {
    let source = InvoiceSource::Photo(image_path.to_string());
    let resp = engine.recognize_image(image_path)?;
    check_and_parse(resp.texts, source)
}

fn check_and_parse(
    text_items: Vec<crate::ocr::OcrTextItem>,
    source: InvoiceSource,
) -> Result<Invoice, String> {
    let doc_type = classify_pdf_document_type(&text_items);
    if doc_type != PdfDocumentType::Invoice {
        return Err(format!("非发票类型: {:?}", doc_type));
    }
    parse_invoice_text(&text_items, source)
}

/// 解析行程单 PDF，返回行程明细集合
/// 当 pdfplumber 可用时优先使用带坐标文本（跳过 OCR）；
/// 否则走 OCR 路径以保留坐标，利用坐标还原表格行列结构；
/// 均失败时回退到纯文本解析。
pub fn parse_itinerary_from_pdf(
    pdf_path: &str,
    engine: &mut OcrEngine,
) -> Result<ItineraryDoc, String> {
    // 优先使用 extract_pdf_column_aware（含 pdfplumber 回退，保留页边界）
    // 多页行程单必须按页解析，否则 Y 坐标重叠导致表格解析失败
    // normalize_columns=true：按全表列网格补全缺竖线的数据行（如天府通第 3 行无竖线，
    // false 时整行合并成 1-cell 导致丢行程）；发票路径保持 false（备注行不能被切碎）
    #[cfg(feature = "pdfplumber")]
    {
        match text_extractor::extract_pdf_column_aware_with_norm(pdf_path, true) {
            Ok(extraction) => {
                let flat_texts: Vec<_> = extraction
                    .pages
                    .iter()
                    .flat_map(|p| p.texts.clone())
                    .collect();
                if text_extractor::has_sufficient_text(&flat_texts, 20) {
                    let doc_type = classify_pdf_document_type(&flat_texts);
                    if doc_type != PdfDocumentType::Itinerary
                        && doc_type != PdfDocumentType::Invoice
                    {
                        return Err(format!("非行程单类型: {:?}", doc_type));
                    }
                    eprintln!(
                        "  [pdfplumber] 列感知提取 {} 个文本项, {} 个原始Word ({} 页)",
                        flat_texts.len(),
                        extraction.raw_words.len(),
                        extraction.pages.len()
                    );

                    // 优先用 find_tables 单元格解析（merged_text 字段完整，不走坐标拆分）
                    let table_itin = parse_itinerary_from_tables(&extraction.tables);
                    if let Some(mut itin) = table_itin {
                        if !itin.is_empty() && !has_incomplete_entries(&itin) {
                            // 单元格文本直接取自 PDF，时间无年份（"06-07 20:17"），
                            // 用顶部"行程起止日期"补全年份，与坐标/文本路径一致（回归修复）
                            let fb_text: String = flat_texts
                                .iter()
                                .map(|t| t.text.as_str())
                                .collect::<Vec<_>>()
                                .join("\n");
                            enrich_itinerary_years(&mut itin, &fb_text);
                            // 服务商列被折行/截断时（如高德"首汽约车 六座商务"只取到"首汽约车"），
                            // 用参考文本按位置补全（cross_validate 仅在截断/空值时修正）
                            cross_validate_amounts(&mut itin, &flat_texts);
                            eprintln!("  [pdfplumber] 单元格表格解析成功，{} 条行程", itin.len());
                            return build_itinerary_doc(itin, &flat_texts, pdf_path);
                        }
                        if !itin.is_empty() {
                            eprintln!("  [pdfplumber] 单元格解析有缺失字段，回退坐标解析");
                        }
                    }

                    let has_coords = flat_texts.iter().any(|t| t.box_coords.is_some());

                    let itineraries = if has_coords {
                        eprintln!("  [pdfplumber] 行程单带坐标，按页 word 级坐标解析");
                        // 关键：用 word_pages（word 级未合并）按页解析，保留单元格坐标
                        let coord_result = parse_itinerary_with_coords_pages_and_fallback(
                            &extraction.word_pages,
                            Some(&flat_texts),
                        );
                        if !coord_result.is_empty() && !has_incomplete_entries(&coord_result) {
                            coord_result
                        } else {
                            if !coord_result.is_empty() {
                                eprintln!("  [pdfplumber] 坐标解析有缺失字段，尝试纯文本解析");
                            } else {
                                eprintln!("  [pdfplumber] 按页坐标解析失败，尝试纯文本解析");
                            }
                            let mut text_result = parse_itinerary_text(&flat_texts);
                            if !text_result.is_empty() {
                                cross_validate_amounts(&mut text_result, &flat_texts);
                                let fb_text: String = flat_texts
                                    .iter()
                                    .map(|t| t.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                enrich_itinerary_years(&mut text_result, &fb_text);
                                if !has_incomplete_entries(&text_result) {
                                    text_result
                                } else {
                                    eprintln!("  [pdfplumber] 纯文本解析仍有缺失字段，回退到 OCR");
                                    let ocr_pages = extract_ocr_text(pdf_path, engine)?;
                                    parse_itinerary_with_coords_pages_and_fallback(
                                        &ocr_pages,
                                        Some(&flat_texts),
                                    )
                                }
                            } else {
                                eprintln!("  [pdfplumber] 纯文本解析失败，回退到 OCR");
                                let ocr_pages = extract_ocr_text(pdf_path, engine)?;
                                parse_itinerary_with_coords_pages_and_fallback(
                                    &ocr_pages,
                                    Some(&flat_texts),
                                )
                            }
                        }
                    } else {
                        let ocr_pages = extract_ocr_text(pdf_path, engine)?;
                        parse_itinerary_with_coords_pages_and_fallback(
                            &ocr_pages,
                            Some(&flat_texts),
                        )
                    };

                    if itineraries.is_empty() {
                        return Err("行程单中未解析到行程明细".to_string());
                    }
                    return build_itinerary_doc(itineraries, &flat_texts, pdf_path);
                }
                eprintln!("  [pdfplumber] 文本不足，回退到 OCR");
            }
            Err(e) => {
                eprintln!("  [pdfplumber] 失败: {}，回退到 OCR", e);
            }
        }
    }

    // 回退路径：OCR（无 pdfplumber 时）
    let ocr_pages = extract_ocr_text(pdf_path, engine)?;
    let ocr_items: Vec<_> = ocr_pages.iter().flat_map(|p| p.texts.clone()).collect();
    let doc_type = classify_pdf_document_type(&ocr_items);
    if doc_type != PdfDocumentType::Itinerary && doc_type != PdfDocumentType::Invoice {
        return Err(format!("非行程单类型: {:?}", doc_type));
    }

    let mut itineraries =
        parse_itinerary_with_coords_pages_and_fallback(&ocr_pages, Some(&ocr_items));

    if itineraries.is_empty() {
        eprintln!("  [OCR] 坐标解析无结果，尝试纯文本回退");
        itineraries = parse_itinerary_text(&ocr_items);
    }

    if itineraries.is_empty() {
        return Err("行程单中未解析到行程明细".to_string());
    }
    if has_incomplete_entries(&itineraries) {
        eprintln!("  [警告] 部分行程有时间字段不完整（OCR 乱码），已保留其余完整条目");
    }
    build_itinerary_doc(itineraries, &ocr_items, pdf_path)
}

/// 构建 ItineraryDoc
fn build_itinerary_doc(
    mut itineraries: Vec<Itinerary>,
    texts: &[crate::ocr::OcrTextItem],
    pdf_path: &str,
) -> Result<ItineraryDoc, String> {
    compute_incomplete_fields(&mut itineraries);
    let printed_total = extract_itinerary_printed_total(texts);
    if let Some(pt) = printed_total {
        cross_validate_with_printed_total(&mut itineraries, pt);
        let file_name = Path::new(pdf_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        return Ok(ItineraryDoc {
            file_name,
            itineraries,
            total_amount: pt,
            printed_total: Some(pt),
        });
    }
    let total_amount: f64 = itineraries.iter().map(|i| i.amount).sum();
    let file_name = Path::new(pdf_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    Ok(ItineraryDoc {
        file_name,
        itineraries,
        total_amount,
        printed_total: None,
    })
}

/// 批量解析目录下所有 PDF（发票+行程单），自动配对
/// 行程单解析结果会被配对到对应的 CityTransport 发票上（按总额匹配）
pub fn parse_all_from_dir(
    dir: &str,
    engine: &mut OcrEngine,
    config: &ExtractionConfig,
) -> ParseResult {
    let mut invoices = Vec::new();
    let mut errors = Vec::new();
    let mut itinerary_docs = Vec::new();

    let pdf_files: Vec<PathBuf> = match Path::new(dir).read_dir() {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "pdf"))
            .map(|e| e.path())
            .collect(),
        Err(_) => {
            return ParseResult {
                invoices,
                errors,
                duplicates: Vec::new(),
            }
        }
    };

    for path in &pdf_files {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        // 先尝试以发票解析
        match parse_invoice_from_pdf(path.to_str().unwrap(), engine, config) {
            Ok(inv) => {
                invoices.push(inv);
                continue;
            }
            Err(_) => {}
        }
        // 发票解析失败，尝试以行程单解析
        match parse_itinerary_from_pdf(path.to_str().unwrap(), engine) {
            Ok(doc) => itinerary_docs.push(doc),
            Err(e) => errors.push((name, e)),
        }
    }

    // 配对：将行程单与 CityTransport 发票关联
    pair_invoices_with_itineraries(&mut invoices, itinerary_docs, 2.0);

    // 批次内按发票号去重
    let duplicates = deduplicate_invoices(&mut invoices);

    ParseResult {
        invoices,
        errors,
        duplicates,
    }
}

/// 批量识别文件列表（发票+行程单），自动匹配行程单到发票
pub fn parse_all_from_files(
    files: &[String],
    engine: &mut OcrEngine,
    config: &ExtractionConfig,
    progress: Option<&dyn Fn(usize, usize, &str)>,
) -> ParseResult {
    let mut invoices = Vec::new();
    let mut errors = Vec::new();
    let mut itinerary_docs = Vec::new();

    for (i, path_str) in files.iter().enumerate() {
        let path = Path::new(path_str);
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if let Some(cb) = progress {
            cb(i, files.len(), &name)
        }
        // 先尝试以发票解析
        match parse_invoice_from_pdf(path_str, engine, config) {
            Ok(inv) => {
                invoices.push(inv);
                continue;
            }
            Err(_) => {}
        }
        // 发票解析失败，尝试以行程单解析
        match parse_itinerary_from_pdf(path_str, engine) {
            Ok(doc) => itinerary_docs.push(doc),
            Err(e) => errors.push((name, e)),
        }
    }

    pair_invoices_with_itineraries(&mut invoices, itinerary_docs, 2.0);

    // 批次内按发票号去重
    let duplicates = deduplicate_invoices(&mut invoices);

    ParseResult {
        invoices,
        errors,
        duplicates,
    }
}

/// 将行程明细配对到对应的发票上（按总额匹配）
/// 匹配成功后自动将发票类别设为 CityTransport
pub fn pair_invoices_with_itineraries(
    invoices: &mut Vec<Invoice>,
    itinerary_docs: Vec<ItineraryDoc>,
    _tolerance: f64, // 仅在没有合计金额时使用
) {
    for doc in itinerary_docs {
        // 如果有印制的合计金额，精确匹配（无需容差）
        if doc.printed_total.is_some() {
            let target = invoices.iter_mut().find(|inv| {
                inv.itineraries.is_empty() && (inv.amount - doc.total_amount).abs() <= 0.01
                // 浮点舍入容差
            });
            if let Some(inv) = target {
                inv.category = InvoiceCategory::CityTransport;
                inv.itineraries = doc.itineraries;
                inv.itinerary_file = Some(doc.file_name.clone());
                continue;
            }
        }
        // 没有合计金额时用容差匹配（回退逻辑）
        let target = invoices.iter_mut().find(|inv| {
            inv.itineraries.is_empty() && (inv.amount - doc.total_amount).abs() <= 2.00
        });
        if let Some(inv) = target {
            inv.category = InvoiceCategory::CityTransport;
            inv.itineraries = doc.itineraries;
            inv.itinerary_file = Some(doc.file_name.clone());
        } else {
            // 没有匹配的发票，创建一张虚拟发票
            let id = uuid::Uuid::new_v4().to_string();
            invoices.push(Invoice {
                id,
                invoice_number: String::new(),
                amount: doc.total_amount,
                seller_name: "市内交通".to_string(),
                item_name: "市内交通".to_string(),
                date: chrono::NaiveDate::default(),
                travel_date: None,
                category: InvoiceCategory::CityTransport,
                source: InvoiceSource::Pdf(doc.file_name.clone()),
                itineraries: doc.itineraries,
                itinerary_file: Some(doc.file_name.clone()),
                remarks: String::new(),
                hotel_detail: None,
                departure_city: None,
                arrival_city: None,
                toll_travel_time: None,
                travel_time: None,
            });
        }
    }
}

#[cfg(all(test, feature = "pdfplumber"))]
mod tests {
    use super::*;
    use crate::parser::cell_extractor::CellInvoiceFields;

    // 回归：build_invoice_from_cells（cell 主路径）必须生成唯一非空 id，
    // 否则前端 matches.find(invoice_id) 全部命中同一 match，分趟/匹配失效
    #[test]
    fn test_build_invoice_from_cells_assigns_unique_id() {
        let fields = CellInvoiceFields {
            seller_name: Some("某公司".to_string()),
            amount: Some(100.0),
            ..Default::default()
        };
        let inv = build_invoice_from_cells(
            fields,
            &None,
            "发票号码 123456789012345678 2025年06月15日",
            InvoiceSource::Pdf("test.pdf".to_string()),
        )
        .expect("build_invoice_from_cells 应成功");
        assert!(!inv.id.is_empty(), "cell 路径发票 id 不应为空");
    }

    // 回归：item_name="保险服务" + item_detail="境内机票航意航延组合险"（含"机票"但"保险"被剥离）
    // 真实样本：15_电子发票_20260522_102238_电子发票.pdf（众安在线财产保险）
    // 完整商品名"*保险服务*境内机票航意航延组合险"，"组合险"换行到第二行
    // 修复前 classify_from_item 返回 Flight，修复后返回 Insurance
    #[test]
    fn test_classify_insurance_item_detail_contains_jipiao() {
        let category = classify_from_item(
            "保险服务",
            "境内机票航意航延组合险",
            "众安在线财产保险股份有限公司",
            "*保险服务*境内机票航意航延组合险 ** 1 59.433962 59.43 6% 3.57",
            "保单号PI157MP260571970064610",
        );
        assert_eq!(category, InvoiceCategory::Insurance);
    }

    // 边界：item_name 为空、item_detail 含"航意"时也应归 Insurance
    #[test]
    fn test_classify_insurance_hangyi_in_detail() {
        let category = classify_from_item(
            "",
            "境内机票航意航延组合险",
            "众安在线财产保险股份有限公司",
            "*保险服务*境内机票航意航延组合险",
            "",
        );
        assert_eq!(category, InvoiceCategory::Insurance);
    }

    // 确保正常机票仍归 Flight（item_name 不含"保险"、item_detail 含"机票"）
    #[test]
    fn test_classify_flight_still_works() {
        let category = classify_from_item("运输服务", "国内机票", "中国国航", "航班号 CA1234", "");
        assert_eq!(category, InvoiceCategory::Flight);
    }

    // 回归：extract_item_detail 从单元格按列聚合文本提取完整商品名（跨行不丢字）
    // 真实样本 column_text: "*保险服务*境内机票航意航延 组合险 合 项目名称 ..."
    // 修复前 take_while 在空白处截断 → "境内机票航意航延"（丢"组合险"）
    // 修复后跳过空白、遇表头词停止 → "境内机票航意航延组合险"
    #[test]
    fn test_extract_item_detail_column_text_cross_line() {
        let cell_text = "*保险服务*境内机票航意航延 组合险 合 项目名称 计 规格型号 ** 单份 位 数 量1 59.433962单 价 ¥ 59.4359.43金额 税率/征税率";
        let detail = extract_item_detail(Some(cell_text), None, &[]);
        assert_eq!(detail.as_deref(), Some("境内机票航意航延组合险"));
    }

    // 回归：extract_item_detail 从 raw_words 合并文本提取（cell_text 为空时回退）
    // 模拟 column_aware_merge_in_bbox 输出：商品名后跟 ** 份 1 ...
    #[test]
    fn test_extract_item_detail_fallback_merged() {
        let merged = "*保险服务*境内机票航意航延 ** 份 1 59.433962 59.43 6% 3.57 组合险 合 计 ¥ ¥ 59.43 3.57";
        let detail = extract_item_detail(Some(merged), None, &[]);
        assert_eq!(detail.as_deref(), Some("境内机票航意航延"));
    }

    // 回归：普通商品名（无跨行）仍正确提取
    #[test]
    fn test_extract_item_detail_normal() {
        let cell_text = "*运输服务*国内机票 份 1 100.00 100.00 9% 9.00";
        let detail = extract_item_detail(Some(cell_text), None, &[]);
        assert_eq!(detail.as_deref(), Some("国内机票"));
    }
}
