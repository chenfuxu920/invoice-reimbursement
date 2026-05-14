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
        } else if trimmed.contains("销售方") && (trimmed.contains("名称") || trimmed.contains("统一社会信用代码")) {
            current_region = "seller";
        } else if trimmed.contains("项目名称") || trimmed.contains("货物或应税劳务") {
            current_region = "items";
        } else if trimmed.contains("价税合计") {
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

pub fn parse_invoice_text(
    texts: &[OcrTextItem],
    source: InvoiceSource,
) -> Result<Invoice, String> {
    let all_text: String = texts
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // 先拆分区域
    let regions = split_into_regions(&all_text);

    // 从对应区域提取字段
    let amount = extract_amount(&regions.total)?;
    let seller_name = extract_seller_name(&regions.seller);
    let item_name = extract_item_name(&regions.items);
    let date = extract_date(&all_text);
    let invoice_number = extract_invoice_number(&regions.header);

    // 分类：优先用商品明细区域，其次用销售方区域
    let category = classify_from_regions(&regions.items, &regions.seller, &item_name, &seller_name);

    // 住宿发票：解析备注栏获取入住/离店日期和天数
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

    Ok(Invoice {
        id: Uuid::new_v4().to_string(),
        invoice_number,
        amount,
        seller_name,
        item_name,
        date,
        category,
        source,
        itineraries: vec![],
        itinerary_file: None,
        remarks: regions.remarks.clone(),
        hotel_detail,
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

    Ok(Invoice {
        id: Uuid::new_v4().to_string(),
        invoice_number: number_field.map(|f| f.value).unwrap_or_default(),
        amount,
        seller_name: seller_field.map(|f| f.value).unwrap_or_default(),
        item_name: item_field.map(|f| f.value).unwrap_or_default(),
        date,
        category,
        source,
        itineraries: vec![],
        itinerary_file: None,
        remarks: String::new(),
        hotel_detail: None,
    })
}

fn try_parse_with_template(
    ocr_output: &OcrStructuredOutput,
    source: &InvoiceSource,
    template: &InvoiceTemplate,
    manager: &TemplateManager,
) -> Result<Invoice, String> {
    let extracted_values = manager.extract_with_template(ocr_output, template)?;

    let invoice_type = InvoiceTypeDetector::detect(ocr_output);
    let category = classify_from_full_text(ocr_output, &None, &None, &invoice_type);

    let mut amount = 0.0f64;
    let mut seller_name = String::new();
    let mut invoice_number = String::new();
    let mut date = chrono::NaiveDate::default();

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
            _ => {}
        }
    }

    Ok(Invoice {
        id: Uuid::new_v4().to_string(),
        invoice_number,
        amount,
        seller_name,
        item_name: String::new(),
        date,
        category,
        source: source.clone(),
        itineraries: vec![],
        itinerary_file: None,
        remarks: String::new(),
        hotel_detail: None,
    })
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

fn extract_amount(text: &str) -> Result<f64, String> {
    let re =
        Regex::new(r"(?:价税合计|合计金额|总金额|金额)[：:￥¥]*\s*([\d,]+\.?\d*)")
            .map_err(|e| e.to_string())?;
    if let Some(caps) = re.captures(text) {
        let amount_str = caps[1].replace(",", "");
        return amount_str.parse::<f64>().map_err(|e| e.to_string());
    }
    // 行程单格式：合计XXX.XX元
    let re_itinerary = Regex::new(r"合计\s*([\d,]+\.?\d*)\s*元").map_err(|e| e.to_string())?;
    if let Some(caps) = re_itinerary.captures(text) {
        let amount_str = caps[1].replace(",", "");
        return amount_str.parse::<f64>().map_err(|e| e.to_string());
    }
    let re2 = Regex::new(r"[￥¥]\s*([\d,]+\.?\d*)").map_err(|e| e.to_string())?;
    let mut max_amount = 0.0f64;
    for cap in re2.captures_iter(text) {
        let v: f64 = cap[1].replace(",", "").parse().unwrap_or(0.0);
        if v > max_amount {
            max_amount = v;
        }
    }
    if max_amount > 0.0 {
        return Ok(max_amount);
    }
    Err("无法识别发票金额".to_string())
}

fn extract_seller_name(text: &str) -> String {
    // 从销售方区域提取名称
    let re = Regex::new(r"名称[：:]\s*(.+?)(?:\s+统一社会信用代码|\s+$)").unwrap();
    if let Some(caps) = re.captures(text) {
        let name = caps[1].trim();
        if !name.is_empty() && name.len() > 2 {
            return name.to_string();
        }
    }
    // 回退：尝试其他模式
    let re2 = Regex::new(r"(?:销售方|收款单位|开票方)[：:]\s*(.+?)(?:\s|$)").unwrap();
    if let Some(caps) = re2.captures(text) {
        return caps[1].trim().to_string();
    }
    String::new()
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
    let re = Regex::new(r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日").unwrap();
    if let Some(caps) = re.captures(text) {
        let y: i32 = caps[1].parse().unwrap_or(2025);
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
    // 反向模式：PDF文字提取时列顺序可能颠倒，号码出现在标签之前
    // 例如：...26512000001728418261发票号码：...
    let re_rev = Regex::new(r"(\d{8,20})\s*发票号码").unwrap();
    if let Some(caps) = re_rev.captures(text) {
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
            keywords: vec!["测试发票".to_string()],
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
}
