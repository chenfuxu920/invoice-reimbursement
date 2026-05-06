use crate::models::invoice::Invoice;
use crate::models::match_result::{MatchResult, MatchType};
use crate::models::payment::PaymentRecord;

const DEFAULT_TOLERANCE: f64 = 1.00;
const MAX_SUBSET_SIZE: usize = 10;
const MAX_PAYMENT_CANDIDATES: usize = 20;

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

    /// One-to-one matching: find a single payment that matches the invoice amount
    /// within the tolerance.
    pub fn match_one_to_one(
        &self,
        invoice: &Invoice,
        payments: &[PaymentRecord],
    ) -> Option<MatchResult> {
        for payment in payments {
            let diff = (invoice.amount - payment.amount).abs();
            if diff <= self.tolerance {
                return Some(MatchResult {
                    invoice_id: invoice.id.clone(),
                    invoice: invoice.clone(),
                    payment_ids: vec![payment.id.clone()],
                    payments: vec![payment.clone()],
                    match_type: MatchType::OneToOne,
                    confidence: 1.0 - (diff / invoice.amount.max(0.01)),
                    amount_diff: diff,
                });
            }
        }
        None
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
            category: InvoiceCategory::Other,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
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
}
