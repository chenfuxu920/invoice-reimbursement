use crate::models::invoice::{Invoice, InvoiceCategory, InvoiceSource};
use crate::ocr::client::OcrTextItem;
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
}
