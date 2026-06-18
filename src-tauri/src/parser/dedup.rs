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
            travel_date: None,
            category: InvoiceCategory::Train,
            source: InvoiceSource::Pdf("test.pdf".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
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
        #[allow(unused_variables)]
        let dupes = deduplicate_invoices(&mut invoices);
        assert_eq!(invoices.len(), 2);
    }

    // ===== 新增测试 =====

    #[test]
    fn test_dedup_no_duplicates() {
        let mut invoices = vec![
            make_invoice("1", "INV001", 100.0, "A"),
            make_invoice("2", "INV002", 200.0, "B"),
            make_invoice("3", "INV003", 300.0, "C"),
        ];
        let dupes = deduplicate_invoices(&mut invoices);
        assert_eq!(invoices.len(), 3);
        assert!(dupes.is_empty());
    }

    #[test]
    fn test_dedup_empty_list() {
        let mut invoices: Vec<Invoice> = vec![];
        let dupes = deduplicate_invoices(&mut invoices);
        assert!(invoices.is_empty());
        assert!(dupes.is_empty());
    }

    #[test]
    fn test_dedup_multiple_duplicates_same_number() {
        let mut invoices = vec![
            make_invoice("1", "INV001", 100.0, "A"),
            make_invoice("2", "INV001", 100.0, "A"),
            make_invoice("3", "INV001", 100.0, "A"),
        ];
        let dupes = deduplicate_invoices(&mut invoices);
        assert_eq!(invoices.len(), 1);
        assert_eq!(dupes.len(), 2);
        // 所有重复项的发票号码都应是 INV001
        for d in &dupes {
            assert_eq!(d, "INV001");
        }
    }

    #[test]
    fn test_dedup_composite_key_different_amounts() {
        // 金额不同但发票号码为空时，不应被去重
        let mut invoices = vec![
            make_invoice("1", "", 100.0, "A"),
            make_invoice("2", "", 200.0, "A"),
        ];
        let dupes = deduplicate_invoices(&mut invoices);
        assert_eq!(invoices.len(), 2);
        assert!(dupes.is_empty());
    }

    #[test]
    fn test_dedup_composite_key_different_dates() {
        // 不同日期不应被去重
        let mut invoices = vec![
            Invoice {
                id: "1".to_string(),
                invoice_number: String::new(),
                amount: 100.0,
                seller_name: "A".to_string(),
                item_name: String::new(),
                date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                travel_date: None,
                category: InvoiceCategory::Train,
                source: InvoiceSource::Pdf("test.pdf".to_string()),
                itineraries: vec![],
                itinerary_file: None,
                remarks: String::new(),
                hotel_detail: None,
                departure_city: None,
                arrival_city: None,
            },
            Invoice {
                id: "2".to_string(),
                invoice_number: String::new(),
                amount: 100.0,
                seller_name: "A".to_string(),
                item_name: String::new(),
                date: NaiveDate::from_ymd_opt(2025, 2, 1).unwrap(),
                travel_date: None,
                category: InvoiceCategory::Train,
                source: InvoiceSource::Pdf("test.pdf".to_string()),
                itineraries: vec![],
                itinerary_file: None,
                remarks: String::new(),
                hotel_detail: None,
                departure_city: None,
                arrival_city: None,
            },
        ];
        let dupes = deduplicate_invoices(&mut invoices);
        assert_eq!(invoices.len(), 2);
        assert!(dupes.is_empty());
    }

    #[test]
    fn test_dedup_mixed_numbered_and_unnumbered() {
        // 混合场景：有号码和无号码
        let mut invoices = vec![
            make_invoice("1", "INV001", 100.0, "A"),
            make_invoice("2", "", 50.0, "B"),   // 无号码，独立
            make_invoice("3", "", 50.0, "B"),   // 无号码，重复
            make_invoice("4", "INV002", 200.0, "C"),
        ];
        let dupes = deduplicate_invoices(&mut invoices);
        assert_eq!(invoices.len(), 3);
        assert_eq!(dupes.len(), 1);
    }

    #[test]
    fn test_dedup_preserves_first_occurrence() {
        let mut invoices = vec![
            make_invoice("first", "INV001", 100.0, "A"),
            make_invoice("second", "INV001", 999.0, "B"),
        ];
        let dupes = deduplicate_invoices(&mut invoices);
        assert_eq!(invoices.len(), 1);
        assert_eq!(invoices[0].id, "first");
        assert_eq!(dupes.len(), 1);
    }
}
