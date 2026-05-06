use crate::models::invoice::Invoice;
use std::collections::HashSet;

/// 去重发票列表，返回重复的发票号码列表
/// 规则：
/// - 有发票号码的按号码去重
/// - 无发票号码的按 (金额+日期+销售方) 组合去重
pub fn deduplicate_invoices(invoices: &mut Vec<Invoice>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();
    let mut unique = Vec::new();

    for invoice in invoices.drain(..) {
        if invoice.invoice_number.is_empty() {
            let key = format!("{}_{}_{}", invoice.amount, invoice.date, invoice.seller_name);
            if seen.insert(key) {
                unique.push(invoice);
            } else {
                duplicates.push(invoice.invoice_number.clone());
            }
        } else if seen.insert(invoice.invoice_number.clone()) {
            unique.push(invoice);
        } else {
            duplicates.push(invoice.invoice_number.clone());
        }
    }

    *invoices = unique;
    duplicates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{InvoiceCategory, InvoiceSource};
    use chrono::NaiveDate;

    fn make_invoice(id: &str, number: &str, amount: f64, seller: &str) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: number.to_string(),
            amount,
            seller_name: seller.to_string(),
            item_name: String::new(),
            date: NaiveDate::from_ymd_opt(2025, 8, 5).unwrap(),
            category: InvoiceCategory::Train,
            source: InvoiceSource::Pdf("test.pdf".to_string()),
            itineraries: vec![],
        }
    }

    #[test]
    fn test_dedup_by_invoice_number() {
        let mut invoices = vec![
            make_invoice("1", "INV001", 100.0, "A"),
            make_invoice("2", "INV001", 100.0, "A"), // 重复
            make_invoice("3", "INV002", 200.0, "B"),
        ];
        let dupes = deduplicate_invoices(&mut invoices);
        assert_eq!(invoices.len(), 2);
        assert_eq!(dupes.len(), 1);
    }

    #[test]
    fn test_dedup_by_composite_key_when_no_number() {
        let mut invoices = vec![
            make_invoice("1", "", 100.0, "A"),
            make_invoice("2", "", 100.0, "A"), // 重复（金额+日期+销售方相同）
            make_invoice("3", "", 100.0, "B"), // 不同销售方
        ];
        let dupes = deduplicate_invoices(&mut invoices);
        assert_eq!(invoices.len(), 2);
    }
}
