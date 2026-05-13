use crate::models::invoice::Invoice;
use crate::models::payment::PaymentRecord;
use crate::models::match_result::{MatchResult, MatchType};

/// 手动创建匹配
pub fn create_manual_match(
    invoice: Invoice,
    payments: Vec<PaymentRecord>,
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
    }
}

/// 取消匹配，释放支付记录
pub fn unmatch_invoice(
    match_result: &MatchResult,
) -> (Invoice, Vec<PaymentRecord>) {
    (match_result.invoice.clone(), match_result.payments.clone())
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
            category: InvoiceCategory::Other,
            source: InvoiceSource::Pdf(String::new()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
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
        let result = create_manual_match(invoice, payments);
        assert!(matches!(result.match_type, MatchType::ManualConfirmed));
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_unmatch() {
        let invoice = make_invoice("inv1", 100.0);
        let payments = vec![make_payment("pay1", 100.0)];
        let result = create_manual_match(invoice, payments);
        let (inv, pays) = unmatch_invoice(&result);
        assert_eq!(inv.id, "inv1");
        assert_eq!(pays.len(), 1);
    }
}
