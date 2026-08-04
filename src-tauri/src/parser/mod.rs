pub mod datetime_util;

pub mod invoice_parser;
pub use invoice_parser::classify_invoice;
pub use invoice_parser::classify_from_full_text;
pub use invoice_parser::parse_invoice_text;

pub mod invoice_type_detector;
pub use invoice_type_detector::InvoiceType;
pub use invoice_type_detector::InvoiceTypeDetector;

pub mod itinerary_parser;
pub use itinerary_parser::parse_itinerary_text;

pub mod dedup;
pub use dedup::deduplicate_invoices;

pub mod link_parser;
pub use link_parser::fetch_invoice_from_link;
pub use link_parser::extract_url_from_qrcode;

pub mod wechat_parser;
pub use wechat_parser::parse_wechat_bill;

pub mod alipay_parser;
pub use alipay_parser::parse_alipay_bill;

/// 自动识别账单类型（微信/支付宝）并解析。
/// 优先按文件内容嗅探：微信账单表头含「交易单号/商户单号」，支付宝含「商家订单号/支付宝」；
/// 内容无法嗅探（如二进制 xlsx）时按扩展名兜底：csv → 支付宝，其余 → 微信。
pub fn parse_bill_auto(file_path: &str) -> Result<Vec<crate::models::payment::PaymentRecord>, String> {
    use std::io::Read;

    let mut detected: Option<bool> = None;
    if let Ok(mut f) = std::fs::File::open(file_path) {
        let mut buf = [0u8; 65536];
        if let Ok(n) = f.read(&mut buf) {
            let (content, _, _) = encoding_rs::GBK.decode(&buf[..n]);
            let content = content.to_string();
            if content.contains("交易单号") || content.contains("商户单号") {
                detected = Some(true);
            } else if content.contains("商家订单号") || content.contains("支付宝") {
                detected = Some(false);
            }
        }
    }

    match detected {
        Some(true) => wechat_parser::parse_wechat_bill(file_path),
        Some(false) => alipay_parser::parse_alipay_bill(file_path),
        None => {
            let ext = std::path::Path::new(file_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext == "csv" {
                alipay_parser::parse_alipay_bill(file_path)
            } else {
                wechat_parser::parse_wechat_bill(file_path)
            }
        }
    }
}

pub mod layout_extractor;

pub mod cell_extractor;
