use chrono::NaiveDate;
use invoice_reimbursement_lib::matching::batch::batch_match;
use invoice_reimbursement_lib::matching::manual::create_manual_match_shared;
use invoice_reimbursement_lib::models::invoice::{
    Invoice, InvoiceCategory, InvoiceSource, Itinerary,
};
use invoice_reimbursement_lib::models::match_result::MatchType;
use invoice_reimbursement_lib::models::payment::{PaymentRecord, PaymentSource};

fn make_trip_invoice(id: &str, amount: f64, itin_time: &str, itin_amount: f64) -> Invoice {
    Invoice {
        id: id.to_string(),
        invoice_number: format!("INV-{}", id),
        amount,
        seller_name: "滴滴出行".to_string(),
        item_name: "市内交通".to_string(),
        date: NaiveDate::parse_from_str(&itin_time[..10], "%Y-%m-%d").unwrap(),
        travel_date: None,
        category: InvoiceCategory::CityTransport,
        source: InvoiceSource::Link("http://example.com".to_string()),
        itineraries: vec![Itinerary {
            date_time: itin_time.to_string(),
            provider: "滴滴".to_string(),
            pickup: "A 站".to_string(),
            dropoff: "B 站".to_string(),
            amount: itin_amount,
            city: String::new(),
            incomplete_fields: vec![],
        }],
        itinerary_file: None,
        remarks: String::new(),
        hotel_detail: None,
        departure_city: None,
        arrival_city: None,
        toll_travel_time: None,
    }
}

fn make_toll_invoice(id: &str, amount: f64, travel_time: &str) -> Invoice {
    Invoice {
        id: id.to_string(),
        invoice_number: format!("TOLL-{}", id),
        amount,
        seller_name: "ETC".to_string(),
        item_name: "通行费".to_string(),
        date: NaiveDate::parse_from_str(&travel_time[..10], "%Y-%m-%d").unwrap(),
        travel_date: None,
        category: InvoiceCategory::Toll,
        source: InvoiceSource::Link("http://example.com".to_string()),
        itineraries: vec![],
        itinerary_file: None,
        remarks: format!("XX站入 XX站出 {}", travel_time),
        hotel_detail: None,
        departure_city: None,
        arrival_city: None,
        toll_travel_time: chrono::NaiveDateTime::parse_from_str(travel_time, "%Y-%m-%d %H:%M:%S")
            .ok(),
    }
}

fn make_payment(id: &str, amount: f64, time: &str) -> PaymentRecord {
    PaymentRecord {
        id: id.to_string(),
        transaction_id: format!("TX-{}", id),
        transaction_time: time.to_string(),
        amount,
        original_amount: amount,
        refund_amount: 0.0,
        discount: 0.0,
        merchant_name: "滴滴出行".to_string(),
        source: PaymentSource::Wechat,
        category: "交通".to_string(),
        payment_method: String::new(),
    }
}

#[test]
fn test_e2e_toll_shared_payment() {
    let trip = make_trip_invoice("inv1", 50.0, "2025-01-15 09:00", 50.0);
    let toll = make_toll_invoice("toll1", 10.0, "2025-01-15 09:30:00");
    let payment = make_payment("p1", 60.0, "2025-01-15 09:35");

    let result = batch_match(&[trip, toll], &[payment], 1.0);

    assert_eq!(result.matched.len(), 2);
    assert_eq!(result.unmatched_invoices.len(), 0);
    assert_eq!(result.unmatched_payments.len(), 0);

    let toll_match = result
        .matched
        .iter()
        .find(|m| m.invoice.category == InvoiceCategory::Toll)
        .unwrap();
    assert_eq!(toll_match.shared_from_invoice_id, Some("inv1".to_string()));
    assert_eq!(toll_match.payments[0].id, "p1");
}

#[test]
fn test_e2e_toll_manual_shared_match() {
    let toll = make_toll_invoice("toll1", 10.0, "2025-01-15 09:30:00");
    let payment = make_payment("p1", 60.0, "2025-01-15 09:35");

    let result =
        create_manual_match_shared(toll, vec![payment], vec![], Some("inv_trip".to_string()));

    assert!(matches!(result.match_type, MatchType::ManualConfirmed));
    assert_eq!(result.shared_from_invoice_id, Some("inv_trip".to_string()));
    assert_eq!(result.shared_payment_ids, vec!["p1".to_string()]);
}

#[test]
fn test_e2e_toll_without_trip_independent_match() {
    // 无行程，但高速费有单独支付，应单独匹配成功
    let toll = make_toll_invoice("toll1", 10.0, "2025-01-15 09:30:00");
    let payment = make_payment("p1", 10.0, "2025-01-15 09:35");

    let result = batch_match(&[toll], &[payment], 1.0);

    assert_eq!(result.matched.len(), 1);
    assert_eq!(result.matched[0].invoice.id, "toll1");
    assert!(result.matched[0].shared_from_invoice_id.is_none());
}

#[test]
fn test_e2e_toll_no_match_goes_unmatched() {
    // 高速费金额与支付不匹配，无行程可关联，应未匹配
    let toll = make_toll_invoice("toll1", 10.0, "2025-01-15 09:30:00");
    let payment = make_payment("p1", 99.0, "2025-01-15 09:35");

    let result = batch_match(&[toll], &[payment], 1.0);

    assert_eq!(result.matched.len(), 0);
    assert_eq!(result.unmatched_invoices.len(), 1);
    assert_eq!(result.unmatched_invoices[0].id, "toll1");
}
