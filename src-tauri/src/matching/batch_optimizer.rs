use crate::models::invoice::Invoice;
use crate::models::match_result::{MatchResult, MatchType};
use crate::models::payment::PaymentRecord;
use super::scoring::{MultiDimensionalScorer, MatchScore};
use super::strategy_selector::{MatchingStrategy, StrategySelector};
use std::collections::HashSet;

const CANDIDATE_THRESHOLD: f64 = 0.5;
const HIGH_CONFIDENCE_THRESHOLD: f64 = 0.7;
const MAX_CANDIDATES_PER_INVOICE: usize = 10;

pub struct BatchMatchOptimizer {
    scorer: MultiDimensionalScorer,
}

impl BatchMatchOptimizer {
    pub fn new() -> Self {
        Self {
            scorer: MultiDimensionalScorer::default_weights(),
        }
    }

    pub fn with_scorer(scorer: MultiDimensionalScorer) -> Self {
        Self { scorer }
    }

    pub fn batch_match(
        &self,
        invoices: &[Invoice],
        payments: &[PaymentRecord],
    ) -> BatchMatchResult {
        let mut matched = Vec::new();
        let mut unmatched_invoices = Vec::new();
        let mut used_payment_ids: HashSet<String> = HashSet::new();
        let mut low_confidence_matches: Vec<(Invoice, MatchScore, PaymentRecord)> = Vec::new();

        for invoice in invoices {
            let strategy = StrategySelector::select(invoice, payments.len());

            let result = match strategy {
                MatchingStrategy::OneToMany => {
                    self.match_one_to_many(invoice, payments, &used_payment_ids)
                }
                _ => {
                    self.match_one_to_one(invoice, payments, &used_payment_ids)
                }
            };

            match result {
                Ok(match_result) => {
                    if match_result.confidence >= HIGH_CONFIDENCE_THRESHOLD {
                        for payment in &match_result.payments {
                            used_payment_ids.insert(payment.id.clone());
                        }
                        matched.push(match_result);
                    } else {
                        low_confidence_matches.push((
                            invoice.clone(),
                            MatchScore {
                                total: match_result.confidence,
                                amount_score: 0.0,
                                merchant_score: 0.0,
                                time_score: 0.0,
                                category_score: 0.0,
                                breakdown: super::scoring::ScoreBreakdown {
                                    amount_diff: match_result.amount_diff,
                                    merchant_similarity: 0.0,
                                    time_diff_hours: 0.0,
                                    category_match: false,
                                },
                            },
                            match_result.payments[0].clone(),
                        ));
                    }
                }
                Err(_) => {
                    unmatched_invoices.push(invoice.clone());
                }
            }
        }

        for (invoice, _score, payment) in low_confidence_matches {
            if !used_payment_ids.contains(&payment.id) {
                used_payment_ids.insert(payment.id.clone());
                matched.push(MatchResult {
                    invoice_id: invoice.id.clone(),
                    invoice: invoice,
                    payment_ids: vec![payment.id.clone()],
                    payments: vec![payment],
                    match_type: MatchType::OneToOne,
                    confidence: CANDIDATE_THRESHOLD,
                    amount_diff: 0.0,
                    itinerary_payment_pairs: vec![],
                });
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

    fn match_one_to_one(
        &self,
        invoice: &Invoice,
        payments: &[PaymentRecord],
        used_payment_ids: &HashSet<String>,
    ) -> Result<MatchResult, MatchError> {
        let mut candidates: Vec<(MatchScore, &PaymentRecord)> = Vec::new();

        for payment in payments.iter().filter(|p| !used_payment_ids.contains(&p.id)) {
            let score = self.scorer.score(invoice, payment);
            if score.total >= CANDIDATE_THRESHOLD {
                candidates.push((score, payment));
            }
        }

        if candidates.is_empty() {
            return Err(MatchError::NoCandidates);
        }

        candidates.sort_by(|a, b| b.0.total.partial_cmp(&a.0.total).unwrap());
        candidates.truncate(MAX_CANDIDATES_PER_INVOICE);

        let (best_score, best_payment) = candidates.first().ok_or(MatchError::NoCandidates)?;

        Ok(MatchResult {
            invoice_id: invoice.id.clone(),
            invoice: invoice.clone(),
            payment_ids: vec![best_payment.id.clone()],
            payments: vec![(*best_payment).clone()],
            match_type: MatchType::OneToOne,
            confidence: best_score.total,
            amount_diff: (invoice.amount - best_payment.total_value()).abs()
                .min((invoice.amount - best_payment.amount).abs()),
            itinerary_payment_pairs: vec![],
        })
    }

    fn match_one_to_many(
        &self,
        invoice: &Invoice,
        payments: &[PaymentRecord],
        used_payment_ids: &HashSet<String>,
    ) -> Result<MatchResult, MatchError> {
        let available_payments: Vec<&PaymentRecord> = payments
            .iter()
            .filter(|p| !used_payment_ids.contains(&p.id) && p.amount <= invoice.amount + 5.0)
            .collect();

        if available_payments.is_empty() {
            return Err(MatchError::NoCandidates);
        }

        let amounts: Vec<f64> = available_payments.iter().map(|p| p.amount).collect();
        let target = invoice.amount;
        let tolerance = 5.0;

        if let Some(indices) = self.find_subset_sum(&amounts, target, tolerance) {
            let matched_payments: Vec<PaymentRecord> =
                indices.iter().map(|&i| available_payments[i].clone()).collect();
            let total: f64 = matched_payments.iter().map(|p| p.amount).sum();

            return Ok(MatchResult {
                invoice_id: invoice.id.clone(),
                invoice: invoice.clone(),
                payment_ids: matched_payments.iter().map(|p| p.id.clone()).collect(),
                payments: matched_payments,
                match_type: MatchType::OneToMany,
                confidence: 1.0 - (target - total).abs().max(0.01) / target.max(0.01),
                amount_diff: (target - total).abs(),
                itinerary_payment_pairs: vec![],
            });
        }

        Err(MatchError::NoMatchingSubset)
    }

    fn find_subset_sum(&self, amounts: &[f64], target: f64, tolerance: f64) -> Option<Vec<usize>> {
        let mut result: Option<Vec<usize>> = None;
        self.search_subset(amounts, target, tolerance, 0, 0.0, &mut Vec::new(), &mut result, 10);
        result
    }

    fn search_subset(
        &self,
        amounts: &[f64],
        target: f64,
        tolerance: f64,
        start: usize,
        current_sum: f64,
        current_indices: &mut Vec<usize>,
        result: &mut Option<Vec<usize>>,
        max_size: usize,
    ) {
        let diff = (target - current_sum).abs();
        if diff <= tolerance && !current_indices.is_empty() {
            *result = Some(current_indices.clone());
            return;
        }

        if current_indices.len() >= max_size {
            return;
        }

        if current_sum > target + tolerance {
            return;
        }

        for i in start..amounts.len() {
            current_indices.push(i);
            self.search_subset(
                amounts,
                target,
                tolerance,
                i + 1,
                current_sum + amounts[i],
                current_indices,
                result,
                max_size,
            );
            if result.is_some() {
                return;
            }
            current_indices.pop();
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchError {
    NoCandidates,
    NoMatchingSubset,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BatchMatchResult {
    pub matched: Vec<MatchResult>,
    pub unmatched_invoices: Vec<Invoice>,
    pub unmatched_payments: Vec<PaymentRecord>,
}

impl BatchMatchResult {
    pub fn total_matched(&self) -> usize {
        self.matched.len()
    }

    pub fn match_rate(&self) -> f64 {
        let total = self.matched.len() + self.unmatched_invoices.len();
        if total == 0 {
            0.0
        } else {
            self.matched.len() as f64 / total as f64
        }
    }

    pub fn high_confidence_matches(&self) -> Vec<&MatchResult> {
        self.matched
            .iter()
            .filter(|m| m.confidence >= HIGH_CONFIDENCE_THRESHOLD)
            .collect()
    }
}

impl Default for BatchMatchOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{InvoiceCategory, InvoiceSource, Itinerary};
    use crate::models::payment::PaymentSource;
    use chrono::NaiveDate;

    fn make_invoice(id: &str, amount: f64, category: InvoiceCategory, seller: &str) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: seller.to_string(),
            item_name: "Test Item".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            travel_date: None,
            category,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
        }
    }

    fn make_city_transport_invoice(id: &str, amount: f64) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "滴滴出行".to_string(),
            item_name: "市内交通".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            travel_date: None,
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![Itinerary {
                date_time: "2025-01-15 09:00".to_string(),
                provider: "滴滴".to_string(),
                pickup: "A站".to_string(),
                dropoff: "B站".to_string(),
                amount: 30.0,
            }],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
        }
    }

    fn make_payment(id: &str, amount: f64, merchant: &str) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            transaction_id: format!("TX-{}", id),
            transaction_time: "2025-01-15 12:00".to_string(),
            amount,
            original_amount: amount,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: merchant.to_string(),
            source: PaymentSource::Wechat,
            category: "交通".to_string(),
            payment_method: String::new(),
        }
    }

    #[test]
    fn test_batch_match_single_match() {
        let optimizer = BatchMatchOptimizer::new();
        let invoice = make_invoice("inv1", 100.0, InvoiceCategory::Hotel, "如家酒店");
        let payment = make_payment("p1", 100.0, "如家酒店");

        let result = optimizer.batch_match(&[invoice], &[payment]);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.unmatched_invoices.len(), 0);
        assert_eq!(result.unmatched_payments.len(), 0);
        assert!(result.matched[0].confidence > 0.7);
    }

    #[test]
    fn test_batch_match_multiple_invoices() {
        let optimizer = BatchMatchOptimizer::new();
        let invoices = vec![
            make_invoice("inv1", 100.0, InvoiceCategory::Hotel, "如家酒店"),
            make_invoice("inv2", 50.0, InvoiceCategory::Meal, "肯德基"),
        ];
        let payments = vec![
            make_payment("p1", 100.0, "如家酒店"),
            make_payment("p2", 50.0, "肯德基"),
        ];

        let result = optimizer.batch_match(&invoices, &payments);

        assert_eq!(result.matched.len(), 2);
        assert_eq!(result.unmatched_invoices.len(), 0);
    }

    #[test]
    fn test_batch_match_one_to_many() {
        let optimizer = BatchMatchOptimizer::new();
        let invoice = make_city_transport_invoice("inv1", 100.0);
        let payments = vec![
            make_payment("p1", 30.0, "滴滴"),
            make_payment("p2", 40.0, "滴滴"),
            make_payment("p3", 30.0, "滴滴"),
        ];

        let result = optimizer.batch_match(&[invoice], &payments);

        assert_eq!(result.matched.len(), 1);
        assert!(matches!(result.matched[0].match_type, MatchType::OneToMany));
    }

    #[test]
    fn test_batch_match_no_match() {
        let optimizer = BatchMatchOptimizer::new();
        let invoice = make_invoice("inv1", 500.0, InvoiceCategory::Hotel, "测试酒店");
        let payment = make_payment("p1", 10.0, "其他商户");

        let result = optimizer.batch_match(&[invoice], &[payment]);

        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 1);
        assert_eq!(result.unmatched_payments.len(), 1);
    }

    #[test]
    fn test_batch_match_partial_match() {
        let optimizer = BatchMatchOptimizer::new();
        let invoices = vec![
            make_invoice("inv1", 100.0, InvoiceCategory::Hotel, "如家酒店"),
            make_invoice("inv2", 999.0, InvoiceCategory::Meal, "测试餐厅"),
        ];
        let payments = vec![make_payment("p1", 100.0, "如家酒店")];

        let result = optimizer.batch_match(&invoices, &payments);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.unmatched_invoices.len(), 1);
        assert_eq!(result.unmatched_payments.len(), 0);
    }

    #[test]
    fn test_batch_match_empty_inputs() {
        let optimizer = BatchMatchOptimizer::new();
        let result = optimizer.batch_match(&[], &[]);
        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 0);
        assert_eq!(result.unmatched_payments.len(), 0);
    }

    #[test]
    fn test_match_rate_calculation() {
        let optimizer = BatchMatchOptimizer::new();
        let invoices = vec![
            make_invoice("inv1", 100.0, InvoiceCategory::Hotel, "如家"),
            make_invoice("inv2", 200.0, InvoiceCategory::Hotel, "汉庭"),
            make_invoice("inv3", 300.0, InvoiceCategory::Hotel, "希尔顿"),
        ];
        let payments = vec![
            make_payment("p1", 100.0, "如家"),
            make_payment("p2", 200.0, "汉庭"),
        ];

        let result = optimizer.batch_match(&invoices, &payments);
        let rate = result.match_rate();

        assert!((rate - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_high_confidence_matches() {
        let optimizer = BatchMatchOptimizer::new();
        let invoices = vec![
            make_invoice("inv1", 100.0, InvoiceCategory::Hotel, "如家酒店"),
        ];
        let payments = vec![make_payment("p1", 100.0, "如家酒店")];

        let result = optimizer.batch_match(&invoices, &payments);
        let high_conf = result.high_confidence_matches();

        assert_eq!(high_conf.len(), 1);
    }

    #[test]
    fn test_match_error_no_candidates() {
        let optimizer = BatchMatchOptimizer::new();
        let invoice = make_invoice("inv1", 100.0, InvoiceCategory::Hotel, "测试");
        let used = HashSet::new();
        let payments = vec![make_payment("p1", 500.0, "其他")];

        let result = optimizer.match_one_to_one(&invoice, &payments, &used);
        assert!(matches!(result, Err(MatchError::NoCandidates)));
    }

    #[test]
    fn test_match_one_to_many_no_subset() {
        let optimizer = BatchMatchOptimizer::new();
        let invoice = make_city_transport_invoice("inv1", 1000.0);
        let used = HashSet::new();
        let payments = vec![make_payment("p1", 10.0, "滴滴")];

        let result = optimizer.match_one_to_many(&invoice, &payments, &used);
        assert!(matches!(result, Err(MatchError::NoMatchingSubset)));
    }

    #[test]
    fn test_batch_match_result_total_matched() {
        let result = BatchMatchResult {
            matched: vec![],
            unmatched_invoices: vec![],
            unmatched_payments: vec![],
        };
        assert_eq!(result.total_matched(), 0);
    }

    #[test]
    fn test_batch_match_with_different_amounts() {
        let optimizer = BatchMatchOptimizer::new();
        let invoice = make_invoice("inv1", 100.0, InvoiceCategory::Hotel, "如家酒店");
        let payments = vec![
            make_payment("p1", 102.0, "如家酒店"),
            make_payment("p2", 100.0, "如家酒店"),
            make_payment("p3", 98.0, "如家酒店"),
        ];

        let result = optimizer.batch_match(&[invoice], &payments);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].payment_ids[0], "p2");
    }

    #[test]
    fn test_batch_match_priority_order() {
        let optimizer = BatchMatchOptimizer::new();
        let invoices = vec![
            make_invoice("inv1", 100.0, InvoiceCategory::Hotel, "如家"),
            make_invoice("inv2", 100.0, InvoiceCategory::Hotel, "汉庭"),
        ];
        let payments = vec![make_payment("p1", 100.0, "如家")];

        let result = optimizer.batch_match(&invoices, &payments);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].invoice_id, "inv1");
    }
}
