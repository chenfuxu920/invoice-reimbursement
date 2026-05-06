use crate::models::invoice::{Invoice, InvoiceCategory, InvoiceSource};
use crate::ocr::OcrTextItem;
use regex::Regex;
use uuid::Uuid;

pub fn parse_invoice_text(
    texts: &[OcrTextItem],
    source: InvoiceSource,
) -> Result<Invoice, String> {
    let all_text: String = texts
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let amount = extract_amount(&all_text)?;
    let seller_name = extract_seller_name(&all_text);
    let item_name = extract_item_name(&all_text);
    let date = extract_date(&all_text);
    let invoice_number = extract_invoice_number(&all_text);
    let category = classify_invoice(&seller_name, &item_name);

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
    })
}

fn extract_amount(text: &str) -> Result<f64, String> {
    // 匹配 "价税合计" "合计金额" "总金额" 等
    let re =
        Regex::new(r"(?:价税合计|合计金额|总金额|金额)[：:￥¥]*\s*([\d,]+\.?\d*)")
            .map_err(|e| e.to_string())?;
    if let Some(caps) = re.captures(text) {
        let amount_str = caps[1].replace(",", "");
        return amount_str.parse::<f64>().map_err(|e| e.to_string());
    }
    // 兜底：找 ¥ 后的最大金额
    let re2 = Regex::new(r"￥\s*([\d,]+\.?\d*)").map_err(|e| e.to_string())?;
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
    let re = Regex::new(r"(?:销售方|收款单位|开票方)[：:]\s*(.+?)(?:\s|$)").unwrap();
    if let Some(caps) = re.captures(text) {
        return caps[1].trim().to_string();
    }
    String::new()
}

fn extract_item_name(text: &str) -> String {
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
    let re = Regex::new(r"(?:发票号码|No)[：:]\s*(\d+)").unwrap();
    if let Some(caps) = re.captures(text) {
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
    fn test_extract_date_cn() {
        let text = "2025年08月05日";
        let date = extract_date(text);
        assert_eq!(
            date,
            chrono::NaiveDate::from_ymd_opt(2025, 8, 5).unwrap()
        );
    }

    // ===== 新增测试 =====

    #[test]
    fn test_classify_train_keywords() {
        // 火车
        assert!(matches!(classify_invoice("铁路客服", ""), InvoiceCategory::Train));
        assert!(matches!(classify_invoice("", "高铁出行"), InvoiceCategory::Train));
        assert!(matches!(classify_invoice("火车票代售", ""), InvoiceCategory::Train));
        assert!(matches!(classify_invoice("", "客运站"), InvoiceCategory::Train));
    }

    #[test]
    fn test_classify_flight_keywords() {
        assert!(matches!(classify_invoice("", "机票"), InvoiceCategory::Flight));
        assert!(matches!(classify_invoice("航空服务", ""), InvoiceCategory::Flight));
        assert!(matches!(classify_invoice("机场餐饮", ""), InvoiceCategory::Flight));
        assert!(matches!(classify_invoice("", "航班延误"), InvoiceCategory::Flight));
    }

    #[test]
    fn test_classify_ticket_change() {
        assert!(matches!(classify_invoice("退票服务", ""), InvoiceCategory::TicketChange));
        assert!(matches!(classify_invoice("", "改签费"), InvoiceCategory::TicketChange));
        assert!(matches!(classify_invoice("保险公司", ""), InvoiceCategory::TicketChange));
    }

    #[test]
    fn test_classify_city_transport() {
        assert!(matches!(classify_invoice("出租汽车", ""), InvoiceCategory::CityTransport));
        assert!(matches!(classify_invoice("", "网约车"), InvoiceCategory::CityTransport));
        assert!(matches!(classify_invoice("高德打车", ""), InvoiceCategory::CityTransport));
        assert!(matches!(classify_invoice("T3出行", ""), InvoiceCategory::CityTransport));
        assert!(matches!(classify_invoice("曹操出行", ""), InvoiceCategory::CityTransport));
    }

    #[test]
    fn test_classify_hotel_keywords() {
        assert!(matches!(classify_invoice("宾馆", ""), InvoiceCategory::Hotel));
        assert!(matches!(classify_invoice("", "住宿费"), InvoiceCategory::Hotel));
        assert!(matches!(classify_invoice("招待所", ""), InvoiceCategory::Hotel));
        assert!(matches!(classify_invoice("", "民宿"), InvoiceCategory::Hotel));
    }

    #[test]
    fn test_classify_meal() {
        assert!(matches!(classify_invoice("餐饮公司", ""), InvoiceCategory::Meal));
        assert!(matches!(classify_invoice("", "饭店"), InvoiceCategory::Meal));
        assert!(matches!(classify_invoice("食品店", ""), InvoiceCategory::Meal));
        assert!(matches!(classify_invoice("", "餐厅"), InvoiceCategory::Meal));
        assert!(matches!(classify_invoice("", "饭馆"), InvoiceCategory::Meal));
    }

    #[test]
    fn test_classify_other() {
        assert!(matches!(classify_invoice("办公用品", "文具"), InvoiceCategory::Other));
        assert!(matches!(classify_invoice("", ""), InvoiceCategory::Other));
    }

    #[test]
    fn test_extract_amount_with_comma() {
        let text = "价税合计：¥1,234.56";
        assert!((extract_amount(text).unwrap() - 1234.56).abs() < 0.01);
    }

    #[test]
    fn test_extract_amount_heji() {
        let text = "合计金额：100.00";
        assert!((extract_amount(text).unwrap() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_extract_amount_total() {
        let text = "总金额：55.50";
        assert!((extract_amount(text).unwrap() - 55.5).abs() < 0.01);
    }

    #[test]
    fn test_extract_amount_yuan_fallback() {
        let text = "消费 ￥30.00 其他 ￥200.00";
        // 应该取最大的金额
        assert!((extract_amount(text).unwrap() - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_extract_amount_failure() {
        let text = "没有任何金额信息";
        assert!(extract_amount(text).is_err());
    }

    #[test]
    fn test_extract_seller_name() {
        let text = "销售方：北京科技有限公司";
        assert_eq!(extract_seller_name(text), "北京科技有限公司");
    }

    #[test]
    fn test_extract_seller_name_colon() {
        let text = "收款单位:上海贸易公司";
        assert_eq!(extract_seller_name(text), "上海贸易公司");
    }

    #[test]
    fn test_extract_seller_name_empty() {
        let text = "没有销售方信息的文本";
        assert_eq!(extract_seller_name(text), "");
    }

    #[test]
    fn test_extract_item_name() {
        let text = "项目名称：交通服务费";
        assert_eq!(extract_item_name(text), "交通服务费");
    }

    #[test]
    fn test_extract_item_name_goods() {
        let text = "货物或应税劳务：餐饮服务";
        assert_eq!(extract_item_name(text), "餐饮服务");
    }

    #[test]
    fn test_extract_item_name_empty() {
        let text = "没有项目名称的文本";
        assert_eq!(extract_item_name(text), "");
    }

    #[test]
    fn test_extract_date_iso_format() {
        let text = "日期 2025-03-15 其他";
        let date = extract_date(text);
        assert_eq!(date, chrono::NaiveDate::from_ymd_opt(2025, 3, 15).unwrap());
    }

    #[test]
    fn test_extract_date_default() {
        let text = "没有日期信息";
        let date = extract_date(text);
        assert_eq!(date, chrono::NaiveDate::default());
    }

    #[test]
    fn test_extract_invoice_number() {
        let text = "发票号码：12345678";
        assert_eq!(extract_invoice_number(text), "12345678");
    }

    #[test]
    fn test_extract_invoice_number_no_prefix() {
        let text = "No：87654321";
        assert_eq!(extract_invoice_number(text), "87654321");
    }

    #[test]
    fn test_extract_invoice_number_empty() {
        let text = "没有发票号码";
        assert_eq!(extract_invoice_number(text), "");
    }

    #[test]
    fn test_parse_invoice_text_full() {
        let texts = vec![
            OcrTextItem { text: "发票号码：12345678".to_string(), confidence: 0.99, box_coords: None },
            OcrTextItem { text: "价税合计：¥200.00".to_string(), confidence: 0.99, box_coords: None },
            OcrTextItem { text: "销售方：滴滴出行".to_string(), confidence: 0.99, box_coords: None },
            OcrTextItem { text: "项目名称：网约车服务".to_string(), confidence: 0.99, box_coords: None },
            OcrTextItem { text: "2025年06月15日".to_string(), confidence: 0.99, box_coords: None },
        ];
        let result = parse_invoice_text(&texts, InvoiceSource::Photo("test.jpg".to_string()));
        assert!(result.is_ok());
        let invoice = result.unwrap();
        assert_eq!(invoice.invoice_number, "12345678");
        assert!((invoice.amount - 200.0).abs() < 0.01);
        assert_eq!(invoice.seller_name, "滴滴出行");
        assert_eq!(invoice.item_name, "网约车服务");
        assert_eq!(invoice.date, chrono::NaiveDate::from_ymd_opt(2025, 6, 15).unwrap());
        assert!(matches!(invoice.category, InvoiceCategory::CityTransport));
    }

    #[test]
    fn test_parse_invoice_text_no_amount() {
        let texts = vec![
            OcrTextItem { text: "发票号码：12345678".to_string(), confidence: 0.99, box_coords: None },
        ];
        let result = parse_invoice_text(&texts, InvoiceSource::Pdf("test.pdf".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invoice_text_empty() {
        let texts: Vec<OcrTextItem> = vec![];
        let result = parse_invoice_text(&texts, InvoiceSource::Link("http://example.com".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_contains_any() {
        assert!(contains_any("滴滴出行", &["滴滴", "高德"]));
        assert!(!contains_any("出租车", &["滴滴", "高德"]));
        assert!(contains_any("hello", &["hello"]));
        assert!(!contains_any("world", &["hello"]));
    }
}
