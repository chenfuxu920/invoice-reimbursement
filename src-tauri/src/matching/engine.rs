use crate::models::invoice::Invoice;
use crate::models::match_result::{MatchResult, MatchType};
use crate::models::payment::PaymentRecord;

const DEFAULT_TOLERANCE: f64 = 1.00;
const MAX_SUBSET_SIZE: usize = 10;
const MAX_PAYMENT_CANDIDATES: usize = 20;

/// 解析支付时间字符串为日期，用于与发票日期比较天数差。
/// 支持 "YYYY-MM-DD HH:MM[:SS]" / "YYYY-MM-DD" / "YYYY/MM/DD ..." 等格式。
fn parse_payment_date(time_str: &str) -> Option<chrono::NaiveDate> {
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
        "%Y/%m/%d",
    ];
    for fmt in &formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(time_str, fmt) {
            return Some(dt.date());
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(time_str, fmt) {
            return Some(d);
        }
    }
    None
}

pub struct MatchEngine {
    pub tolerance: f64,
}

impl MatchEngine {
    pub fn new(tolerance: f64) -> Self {
        Self { tolerance }
    }

    pub fn default() -> Self {
        Self::new(DEFAULT_TOLERANCE)
    }

    /// One-to-one matching: find the single payment closest to the invoice amount
    /// within the tolerance. 当多笔支付都落在金额容差内时，优先选时间与发票日期
    /// 最近的；天数差相同时再按金额差决胜。
    pub fn match_one_to_one(
        &self,
        invoice: &Invoice,
        payments: &[PaymentRecord],
    ) -> Option<MatchResult> {
        // (days_diff, amount_diff, payment)
        let mut best: Option<(i64, f64, &PaymentRecord)> = None;
        for payment in payments {
            let diff_total = (invoice.amount - payment.total_value()).abs();
            let diff_amount = (invoice.amount - payment.amount).abs();
            let diff = diff_total.min(diff_amount);
            if diff > self.tolerance {
                continue;
            }
            let days_diff = parse_payment_date(&payment.transaction_time)
                .map(|pd| (invoice.date - pd).num_days().abs())
                .unwrap_or(i64::MAX);
            let is_better = match best {
                None => true,
                Some((best_days, best_diff, _)) => {
                    days_diff < best_days || (days_diff == best_days && diff < best_diff)
                }
            };
            if is_better {
                best = Some((days_diff, diff, payment));
            }
        }
        best.map(|(_, diff, payment)| MatchResult {
            invoice_id: invoice.id.clone(),
            invoice: invoice.clone(),
            payment_ids: vec![payment.id.clone()],
            payments: vec![payment.clone()],
            match_type: MatchType::OneToOne,
            confidence: 1.0 - (diff / invoice.amount.max(0.01)),
            amount_diff: diff,
            itinerary_payment_pairs: vec![],
            shared_payment_ids: vec![],
            shared_from_invoice_id: None,
        })
    }

    /// One-to-many matching: find a subset of payments whose sum matches
    /// the invoice amount within the tolerance.
    /// Useful for taxi scenarios where one invoice covers multiple rides.
    pub fn match_one_to_many(
        &self,
        invoice: &Invoice,
        payments: &[PaymentRecord],
    ) -> Option<MatchResult> {
        let target = invoice.amount;
        let candidates: Vec<&PaymentRecord> = payments
            .iter()
            .filter(|p| p.amount <= target + self.tolerance)
            .take(MAX_PAYMENT_CANDIDATES)
            .collect();

        if candidates.is_empty() {
            return None;
        }

        if let Some(indices) = self.subset_sum_match(target, &candidates) {
            let matched_payments: Vec<PaymentRecord> =
                indices.iter().map(|&i| candidates[i].clone()).collect();
            let total: f64 = matched_payments.iter().map(|p| p.amount).sum();
            let diff = (invoice.amount - total).abs();
            let payment_ids: Vec<String> = matched_payments.iter().map(|p| p.id.clone()).collect();

            return Some(MatchResult {
                invoice_id: invoice.id.clone(),
                invoice: invoice.clone(),
                payment_ids,
                payments: matched_payments,
                match_type: MatchType::OneToMany,
                confidence: 1.0 - (diff / invoice.amount.max(0.01)),
                amount_diff: diff,
                itinerary_payment_pairs: vec![],
                shared_payment_ids: vec![],
                shared_from_invoice_id: None,
            });
        }

        None
    }

    /// Subset sum matching: find a subset of candidates whose sum is within
    /// tolerance of the target amount. Limits subset size to MAX_SUBSET_SIZE
    /// and candidate count to MAX_PAYMENT_CANDIDATES.
    fn subset_sum_match(
        &self,
        target: f64,
        candidates: &[&PaymentRecord],
    ) -> Option<Vec<usize>> {
        let amounts: Vec<f64> = candidates.iter().map(|p| p.amount).collect();
        let mut result: Option<Vec<usize>> = None;

        self.search_subset(
            &amounts,
            target,
            0,
            0.0,
            &mut Vec::new(),
            &mut result,
        );

        result
    }

    /// Recursive subset sum search with pruning.
    fn search_subset(
        &self,
        amounts: &[f64],
        target: f64,
        start: usize,
        current_sum: f64,
        current_indices: &mut Vec<usize>,
        result: &mut Option<Vec<usize>>,
    ) {
        // Check if current subset sum is within tolerance
        let diff = (target - current_sum).abs();
        if diff <= self.tolerance && !current_indices.is_empty() {
            *result = Some(current_indices.clone());
            return;
        }

        // Pruning: subset too large or overshot too much
        if current_indices.len() >= MAX_SUBSET_SIZE {
            return;
        }

        // Pruning: if current_sum already exceeds target beyond tolerance, stop
        if current_sum > target + self.tolerance {
            return;
        }

        for i in start..amounts.len() {
            current_indices.push(i);
            self.search_subset(
                amounts,
                target,
                i + 1,
                current_sum + amounts[i],
                current_indices,
                result,
            );
            // If we found a result, stop searching
            if result.is_some() {
                return;
            }
            current_indices.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{InvoiceCategory, InvoiceSource};
    use crate::models::payment::PaymentSource;
    use chrono::NaiveDate;

    fn make_invoice(id: &str, amount: f64) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "Test Seller".to_string(),
            item_name: "Test Item".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            travel_date: None,
            category: InvoiceCategory::Other,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
                        toll_travel_time: None,
        }
    }

    fn make_payment(id: &str, amount: f64) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            transaction_id: format!("TX-{}", id),
            transaction_time: "2025-01-01 12:00".to_string(),
            amount,
            original_amount: amount,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: "Test Merchant".to_string(),
            source: PaymentSource::Wechat,
            category: "交通".to_string(),
            payment_method: String::new(),
        }
    }

    fn make_invoice_at(id: &str, amount: f64, date: &str) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "Test Seller".to_string(),
            item_name: "Test Item".to_string(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            travel_date: None,
            category: InvoiceCategory::Other,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
                        toll_travel_time: None,
        }
    }

    fn make_payment_at(id: &str, amount: f64, time: &str) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            transaction_id: format!("TX-{}", id),
            transaction_time: time.to_string(),
            amount,
            original_amount: amount,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: "Test Merchant".to_string(),
            source: PaymentSource::Wechat,
            category: "交通".to_string(),
            payment_method: String::new(),
        }
    }

    #[test]
    fn test_one_to_one_match_success() {
        let engine = MatchEngine::default();
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![
            make_payment("p1", 50.00),
            make_payment("p2", 100.50), // within 1.00 tolerance
            make_payment("p3", 200.00),
        ];

        let result = engine.match_one_to_one(&invoice, &payments).unwrap();
        assert_eq!(result.invoice_id, "inv1");
        assert_eq!(result.payment_ids, vec!["p2".to_string()]);
        assert!(matches!(result.match_type, MatchType::OneToOne));
        assert_eq!(result.amount_diff, 0.50);
        assert!(result.confidence > 0.0 && result.confidence <= 1.0);
    }

    #[test]
    fn test_one_to_one_match_failure() {
        let engine = MatchEngine::default();
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![
            make_payment("p1", 50.00),
            make_payment("p2", 102.00), // beyond 1.00 tolerance
            make_payment("p3", 200.00),
        ];

        let result = engine.match_one_to_one(&invoice, &payments);
        assert!(result.is_none());
    }

    #[test]
    fn test_one_to_many_match() {
        let engine = MatchEngine::default();
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![
            make_payment("p1", 30.00),
            make_payment("p2", 40.00),
            make_payment("p3", 30.50), // 30 + 40 + 30.50 = 100.50, diff = 0.50
            make_payment("p4", 500.00),
        ];

        let result = engine.match_one_to_many(&invoice, &payments).unwrap();
        assert_eq!(result.invoice_id, "inv1");
        assert!(matches!(result.match_type, MatchType::OneToMany));
        assert_eq!(result.payment_ids.len(), 3);
        let total: f64 = result.payments.iter().map(|p| p.amount).sum();
        assert!((total - 100.50).abs() < 0.01);
        assert!(result.amount_diff <= 1.00);
    }

    #[test]
    fn test_default_tolerance() {
        let engine = MatchEngine::default();
        assert!((engine.tolerance - 1.00).abs() < f64::EPSILON);
    }

    #[test]
    fn test_custom_tolerance() {
        let engine = MatchEngine::new(0.50);
        assert!((engine.tolerance - 0.50).abs() < f64::EPSILON);

        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![make_payment("p1", 100.30)];

        // Should match with tolerance 0.50
        let result = engine.match_one_to_one(&invoice, &payments);
        assert!(result.is_some());

        let engine_strict = MatchEngine::new(0.10);
        // Should NOT match with tolerance 0.10
        let result = engine_strict.match_one_to_one(&invoice, &payments);
        assert!(result.is_none());
    }

    #[test]
    fn test_one_to_many_no_match() {
        let engine = MatchEngine::default();
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![
            make_payment("p1", 10.00),
            make_payment("p2", 15.00),
        ];

        let result = engine.match_one_to_many(&invoice, &payments);
        assert!(result.is_none());
    }

    #[test]
    fn test_one_to_one_exact_match() {
        let engine = MatchEngine::default();
        let invoice = make_invoice("inv1", 88.88);
        let payments = vec![make_payment("p1", 88.88)];

        let result = engine.match_one_to_one(&invoice, &payments).unwrap();
        assert_eq!(result.amount_diff, 0.0);
        assert!((result.confidence - 1.0).abs() < f64::EPSILON);
    }

    // ===== 新增测试 =====

    #[test]
    fn test_one_to_one_empty_payments() {
        let engine = MatchEngine::default();
        let invoice = make_invoice("inv1", 100.00);
        let payments: Vec<PaymentRecord> = vec![];

        let result = engine.match_one_to_one(&invoice, &payments);
        assert!(result.is_none());
    }

    #[test]
    fn test_one_to_one_within_tolerance() {
        let engine = MatchEngine::new(0.50);
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![make_payment("p1", 100.30)];

        let result = engine.match_one_to_one(&invoice, &payments);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!((r.amount_diff - 0.30).abs() < 0.01);
    }

    #[test]
    fn test_one_to_one_beyond_tolerance() {
        let engine = MatchEngine::new(0.10);
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![make_payment("p1", 100.50)];

        let result = engine.match_one_to_one(&invoice, &payments);
        assert!(result.is_none());
    }

    #[test]
    fn test_one_to_one_returns_closest_match() {
        let engine = MatchEngine::new(1.00);
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![
            make_payment("p1", 99.30),
            make_payment("p2", 100.50),
            make_payment("p3", 99.80),
        ];

        let result = engine.match_one_to_one(&invoice, &payments).unwrap();
        // p3 (99.80, diff=0.20) is closer than p1 (99.30, diff=0.70) and p2 (100.50, diff=0.50)
        assert_eq!(result.payment_ids[0], "p3");
    }

    #[test]
    fn test_one_to_many_exact_sum() {
        let engine = MatchEngine::default();
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![
            make_payment("p1", 40.00),
            make_payment("p2", 60.00),
        ];

        let result = engine.match_one_to_many(&invoice, &payments).unwrap();
        assert!(matches!(result.match_type, MatchType::OneToMany));
        assert_eq!(result.payment_ids.len(), 2);
        let total: f64 = result.payments.iter().map(|p| p.amount).sum();
        assert!((total - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_one_to_many_three_payments() {
        let engine = MatchEngine::default();
        let invoice = make_invoice("inv1", 150.00);
        let payments = vec![
            make_payment("p1", 30.00),
            make_payment("p2", 50.00),
            make_payment("p3", 70.00),
            make_payment("p4", 999.00),
        ];

        let result = engine.match_one_to_many(&invoice, &payments).unwrap();
        assert!(matches!(result.match_type, MatchType::OneToMany));
        let total: f64 = result.payments.iter().map(|p| p.amount).sum();
        assert!((total - 150.0).abs() < 1.01);
    }

    #[test]
    fn test_one_to_many_empty_payments() {
        let engine = MatchEngine::default();
        let invoice = make_invoice("inv1", 100.00);
        let payments: Vec<PaymentRecord> = vec![];

        let result = engine.match_one_to_many(&invoice, &payments);
        assert!(result.is_none());
    }

    #[test]
    fn test_one_to_many_all_payments_too_large() {
        let engine = MatchEngine::default();
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![
            make_payment("p1", 200.00),
            make_payment("p2", 300.00),
        ];

        let result = engine.match_one_to_many(&invoice, &payments);
        assert!(result.is_none());
    }

    #[test]
    fn test_match_confidence_range() {
        let engine = MatchEngine::new(1.00);
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![make_payment("p1", 99.50)];

        let result = engine.match_one_to_one(&invoice, &payments).unwrap();
        assert!(result.confidence > 0.0 && result.confidence <= 1.0);
    }

    #[test]
    fn test_tolerance_zero_no_match_on_diff() {
        let engine = MatchEngine::new(0.0);
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![make_payment("p1", 100.01)];

        let result = engine.match_one_to_one(&invoice, &payments);
        assert!(result.is_none());
    }

    #[test]
    fn test_tolerance_zero_exact_match() {
        let engine = MatchEngine::new(0.0);
        let invoice = make_invoice("inv1", 100.00);
        let payments = vec![make_payment("p1", 100.00)];

        let result = engine.match_one_to_one(&invoice, &payments);
        assert!(result.is_some());
    }

    #[test]
    fn test_one_to_one_prefers_closer_time() {
        // 金额都在容差内时，优先选时间最近的支付，而非金额差最小的
        let engine = MatchEngine::new(1.00);
        let invoice = make_invoice_at("inv1", 100.00, "2025-01-15");
        let payments = vec![
            make_payment_at("p1", 100.50, "2025-01-15 12:00"), // 同一天，金额差0.50
            make_payment_at("p2", 99.90, "2025-01-10 12:00"),  // 5天前，金额差0.10
        ];

        let result = engine.match_one_to_one(&invoice, &payments).unwrap();
        // 应优先时间近的 p1，而非金额差更小的 p2
        assert_eq!(result.payment_ids[0], "p1");
    }

    #[test]
    fn test_one_to_one_time_tie_breaks_by_amount() {
        // 时间相同（同一天）时，按金额差决胜
        let engine = MatchEngine::new(1.00);
        let invoice = make_invoice_at("inv1", 100.00, "2025-01-15");
        let payments = vec![
            make_payment_at("p1", 100.50, "2025-01-15 12:00"), // 同一天，金额差0.50
            make_payment_at("p2", 99.90, "2025-01-15 18:00"),  // 同一天，金额差0.10
        ];

        let result = engine.match_one_to_one(&invoice, &payments).unwrap();
        // 同一天，按金额差选 p2
        assert_eq!(result.payment_ids[0], "p2");
    }
}

