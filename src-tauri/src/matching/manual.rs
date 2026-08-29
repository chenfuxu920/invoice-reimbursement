use crate::models::invoice::Invoice;
use crate::models::match_result::{ItineraryPaymentPair, MatchResult, MatchType};
use crate::models::payment::PaymentRecord;

/// 手动创建匹配
/// itinerary_payment_pairs：行程-支付显式配对（市内交通一对多场景）；
///   非行程场景传空 Vec。
pub fn create_manual_match(
    invoice: Invoice,
    payments: Vec<PaymentRecord>,
    itinerary_payment_pairs: Vec<ItineraryPaymentPair>,
) -> MatchResult {
    let total: f64 = payments.iter().map(|p| p.amount).sum();
    let diff = (invoice.amount - total).abs();

    MatchResult {
        invoice_id: invoice.id.clone(),
        invoice,
        payment_ids: payments.iter().map(|p| p.id.clone()).collect(),
        payments,
        match_type: MatchType::ManualConfirmed,
        confidence: if diff == 0.0 { 1.0 } else { 0.8 },
        amount_diff: diff,
        itinerary_payment_pairs,
        shared_payment_ids: vec![],
        shared_from_invoice_id: None,
    }
}

/// 取消匹配，释放支付记录
pub fn unmatch_invoice(match_result: &MatchResult) -> (Invoice, Vec<PaymentRecord>) {
    (match_result.invoice.clone(), match_result.payments.clone())
}

/// 手动创建共享匹配（高速费复用行程支付）
/// shared_from_invoice_id：共享来源的行程发票ID
pub fn create_manual_match_shared(
    invoice: Invoice,
    payments: Vec<PaymentRecord>,
    itinerary_payment_pairs: Vec<ItineraryPaymentPair>,
    shared_from_invoice_id: Option<String>,
) -> MatchResult {
    let total: f64 = payments.iter().map(|p| p.amount).sum();
    let diff = (invoice.amount - total).abs();
    let payment_ids: Vec<String> = payments.iter().map(|p| p.id.clone()).collect();

    MatchResult {
        invoice_id: invoice.id.clone(),
        invoice,
        payment_ids: payment_ids.clone(),
        payments,
        match_type: MatchType::ManualConfirmed,
        confidence: if diff == 0.0 { 1.0 } else { 0.8 },
        amount_diff: diff,
        itinerary_payment_pairs,
        shared_payment_ids: if shared_from_invoice_id.is_some() {
            payment_ids
        } else {
            vec![]
        },
        shared_from_invoice_id,
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
            invoice_number: String::new(),
            amount,
            seller_name: String::new(),
            item_name: String::new(),
            date: NaiveDate::default(),
            travel_date: None,
            category: InvoiceCategory::Other,
            source: InvoiceSource::Pdf(String::new()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: None,
            travel_time: None,
        }
    }

    fn make_payment(id: &str, amount: f64) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            transaction_id: String::new(),
            transaction_time: String::new(),
            amount,
            original_amount: amount,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: String::new(),
            source: PaymentSource::Wechat,
            category: String::new(),
            payment_method: String::new(),
        }
    }

    #[test]
    fn test_manual_match() {
        let invoice = make_invoice("inv1", 100.0);
        let payments = vec![make_payment("pay1", 100.0)];
        let result = create_manual_match(invoice, payments, vec![]);
        assert!(matches!(result.match_type, MatchType::ManualConfirmed));
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_unmatch() {
        let invoice = make_invoice("inv1", 100.0);
        let payments = vec![make_payment("pay1", 100.0)];
        let result = create_manual_match(invoice, payments, vec![]);
        let (inv, pays) = unmatch_invoice(&result);
        assert_eq!(inv.id, "inv1");
        assert_eq!(pays.len(), 1);
    }

    #[test]
    fn test_manual_match_with_shared_payment() {
        let invoice = make_invoice("toll1", 10.0);
        let payments = vec![make_payment("pay1", 60.0)];
        let result =
            create_manual_match_shared(invoice, payments, vec![], Some("inv_trip".to_string()));
        assert_eq!(result.shared_from_invoice_id, Some("inv_trip".to_string()));
        assert_eq!(result.shared_payment_ids, vec!["pay1".to_string()]);
        assert!(matches!(result.match_type, MatchType::ManualConfirmed));
    }
}
