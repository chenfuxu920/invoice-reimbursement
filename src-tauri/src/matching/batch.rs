use crate::models::invoice::{Invoice, InvoiceCategory};
use crate::models::match_result::MatchResult;
use crate::models::payment::PaymentRecord;
use super::engine::MatchEngine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMatchResult {
    pub matched: Vec<MatchResult>,
    pub unmatched_invoices: Vec<Invoice>,
    pub unmatched_payments: Vec<PaymentRecord>,
}

pub fn batch_match(
    invoices: &[Invoice],
    payments: &[PaymentRecord],
    tolerance: f64,
) -> BatchMatchResult {
    let engine = MatchEngine::new(tolerance);
    let mut matched = Vec::new();
    let mut unmatched_invoices = Vec::new();
    let mut used_payment_ids: Vec<String> = Vec::new();

    for invoice in invoices {
        let available_payments: Vec<PaymentRecord> = payments
            .iter()
            .filter(|p| !used_payment_ids.contains(&p.id))
            .cloned()
            .collect();

        let result = if invoice.category == InvoiceCategory::CityTransport
            && !invoice.itineraries.is_empty()
        {
            // 打车场景：一对多匹配
            engine.match_one_to_many(invoice, &available_payments)
        } else {
            // 普通场景：一对一匹配
            engine.match_one_to_one(invoice, &available_payments)
        };

        if let Some(match_result) = result {
            for pid in &match_result.payment_ids {
                used_payment_ids.push(pid.clone());
            }
            matched.push(match_result);
        } else {
            unmatched_invoices.push(invoice.clone());
        }
    }

    let unmatched_payments: Vec<PaymentRecord> = payments
        .iter()
        .filter(|p| !used_payment_ids.contains(&p.id))
        .cloned()
        .collect();

    BatchMatchResult {
        matched,
        unmatched_invoices,
        unmatched_payments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{InvoiceCategory, InvoiceSource, Itinerary};
    use crate::models::match_result::MatchType;
    use crate::models::payment::PaymentSource;
    use chrono::NaiveDate;

    fn make_invoice(id: &str, amount: f64, category: InvoiceCategory) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "Test Seller".to_string(),
            item_name: "Test Item".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            category,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
        }
    }

    fn make_city_transport_invoice(id: &str, amount: f64) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "滴滴出行".to_string(),
            item_name: "市内交通".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![Itinerary {
                date_time: "2025-01-01 09:00".to_string(),
                provider: "滴滴".to_string(),
                pickup: "A站".to_string(),
                dropoff: "B站".to_string(),
                amount: 30.00,
            }],
        }
    }

    fn make_payment(id: &str, amount: f64) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            transaction_id: format!("TX-{}", id),
            transaction_time: "2025-01-01 12:00".to_string(),
            amount,
            merchant_name: "Test Merchant".to_string(),
            source: PaymentSource::Wechat,
            category: "交通".to_string(),
        }
    }

    #[test]
    fn test_batch_match_mixed_invoices() {
        // 2张普通发票 + 1张打车发票
        let invoices = vec![
            make_invoice("inv1", 100.00, InvoiceCategory::Hotel),
            make_invoice("inv2", 50.00, InvoiceCategory::Meal),
            make_city_transport_invoice("inv3", 100.00),
        ];

        // 5笔支付
        let payments = vec![
            make_payment("p1", 100.00),  // 匹配 inv1 (一对一)
            make_payment("p2", 50.00),   // 匹配 inv2 (一对一)
            make_payment("p3", 30.00),   // 匹配 inv3 (一对多组合)
            make_payment("p4", 40.00),   // 匹配 inv3 (一对多组合)
            make_payment("p5", 30.50),   // 匹配 inv3 (一对多组合) → 30+40+30.50=100.50
        ];

        let result = batch_match(&invoices, &payments, 1.00);

        // 应该全部匹配成功
        assert_eq!(result.matched.len(), 3);
        assert_eq!(result.unmatched_invoices.len(), 0);
        assert_eq!(result.unmatched_payments.len(), 0);

        // 验证一对一匹配的发票
        let one_to_one_results: Vec<&MatchResult> = result
            .matched
            .iter()
            .filter(|r| matches!(r.match_type, MatchType::OneToOne))
            .collect();
        assert_eq!(one_to_one_results.len(), 2);

        // 验证一对多匹配的打车发票
        let one_to_many_results: Vec<&MatchResult> = result
            .matched
            .iter()
            .filter(|r| matches!(r.match_type, MatchType::OneToMany))
            .collect();
        assert_eq!(one_to_many_results.len(), 1);
        assert_eq!(one_to_many_results[0].invoice_id, "inv3");
        assert_eq!(one_to_many_results[0].payment_ids.len(), 3);

        let total: f64 = one_to_many_results[0].payments.iter().map(|p| p.amount).sum();
        assert!((total - 100.50).abs() < 0.01);
    }

    #[test]
    fn test_batch_match_no_match() {
        let invoices = vec![
            make_invoice("inv1", 500.00, InvoiceCategory::Hotel),
            make_invoice("inv2", 999.00, InvoiceCategory::Meal),
        ];

        let payments = vec![
            make_payment("p1", 10.00),
            make_payment("p2", 20.00),
        ];

        let result = batch_match(&invoices, &payments, 1.00);

        // 没有匹配
        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 2);
        assert_eq!(result.unmatched_payments.len(), 2);
    }

    #[test]
    fn test_batch_match_partial_match() {
        let invoices = vec![
            make_invoice("inv1", 100.00, InvoiceCategory::Hotel),
            make_invoice("inv2", 999.00, InvoiceCategory::Meal),
        ];

        let payments = vec![
            make_payment("p1", 100.00), // 匹配 inv1
            make_payment("p2", 20.00),  // 无匹配
        ];

        let result = batch_match(&invoices, &payments, 1.00);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.unmatched_invoices.len(), 1);
        assert_eq!(result.unmatched_payments.len(), 1);
        assert_eq!(result.unmatched_invoices[0].id, "inv2");
        assert_eq!(result.unmatched_payments[0].id, "p2");
    }

    #[test]
    fn test_batch_match_city_transport_without_itineraries_uses_one_to_one() {
        // 打车类别但没有行程，应走一对一匹配
        let mut invoice = make_invoice("inv1", 100.00, InvoiceCategory::CityTransport);
        invoice.itineraries = vec![]; // 无行程

        let payments = vec![
            make_payment("p1", 30.00),
            make_payment("p2", 70.00),  // 30+70=100 但不应该一对多
            make_payment("p3", 100.00), // 精确匹配
        ];

        let result = batch_match(&[invoice], &payments, 1.00);

        assert_eq!(result.matched.len(), 1);
        assert!(matches!(result.matched[0].match_type, MatchType::OneToOne));
        assert_eq!(result.matched[0].payment_ids, vec!["p3".to_string()]);
    }

    #[test]
    fn test_batch_match_empty_inputs() {
        let result = batch_match(&[], &[], 1.00);
        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 0);
        assert_eq!(result.unmatched_payments.len(), 0);

        let invoices = vec![make_invoice("inv1", 100.00, InvoiceCategory::Other)];
        let result = batch_match(&invoices, &[], 1.00);
        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 1);

        let payments = vec![make_payment("p1", 100.00)];
        let result = batch_match(&[], &payments, 1.00);
        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_payments.len(), 1);
    }
}
