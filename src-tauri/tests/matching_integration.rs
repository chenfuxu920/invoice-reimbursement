use invoice_reimbursement_lib::models::invoice::{Invoice, InvoiceCategory, InvoiceSource, Itinerary};
use invoice_reimbursement_lib::models::payment::{PaymentRecord, PaymentSource};
use invoice_reimbursement_lib::models::match_result::{MatchResult, MatchType};
use invoice_reimbursement_lib::matching::engine::MatchEngine;
use invoice_reimbursement_lib::matching::batch::batch_match;
use invoice_reimbursement_lib::matching::manual::{create_manual_match, unmatch_invoice};
use invoice_reimbursement_lib::pdf::form_builder::build_reimbursement_form;
use chrono::NaiveDate;

// ===== Helper functions =====

fn make_invoice(id: &str, amount: f64, category: InvoiceCategory) -> Invoice {
    Invoice {
        id: id.to_string(),
        invoice_number: format!("INV-{}", id),
        amount,
        seller_name: "测试商家".to_string(),
        item_name: "测试项目".to_string(),
        date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        travel_date: None,
        category,
        source: InvoiceSource::Photo("test.jpg".to_string()),
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
        source: InvoiceSource::Photo("taxi.jpg".to_string()),
        itineraries: vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "北京站".to_string(),
            dropoff: "国贸".to_string(),
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
        discount: 0.0,
        merchant_name: merchant.to_string(),
        source: PaymentSource::Wechat,
        category: "交通".to_string(),
        payment_method: String::new(),
        original_amount: 0.0,
        refund_amount: 0.0,
    }
}

// ===== Full match flow integration tests =====

/// Test the complete matching flow: create invoices and payments, match them,
/// and verify the results are correct.
#[test]
fn test_full_match_flow() {
    let invoices = vec![
        make_invoice("inv1", 100.0, InvoiceCategory::Hotel),
        make_invoice("inv2", 553.0, InvoiceCategory::Train),
    ];

    let payments = vec![
        make_payment("pay1", 100.0, "测试酒店"),
        make_payment("pay2", 553.0, "12306"),
    ];

    let engine = MatchEngine::new(1.0);

    // Match first invoice
    let result1 = engine.match_one_to_one(&invoices[0], &payments);
    assert!(result1.is_some(), "Hotel invoice should match a payment");
    let r1 = result1.unwrap();
    assert_eq!(r1.invoice_id, "inv1");
    assert_eq!(r1.payment_ids.len(), 1);
    assert_eq!(r1.payment_ids[0], "pay1");
    assert!(matches!(r1.match_type, MatchType::OneToOne));
    assert!((r1.amount_diff).abs() < 0.01);

    // Match second invoice
    let result2 = engine.match_one_to_one(&invoices[1], &payments);
    assert!(result2.is_some(), "Train invoice should match a payment");
    let r2 = result2.unwrap();
    assert_eq!(r2.invoice_id, "inv2");
    assert_eq!(r2.payment_ids[0], "pay2");
}

/// Test batch matching with mixed invoice types (one-to-one + one-to-many).
#[test]
fn test_batch_match_full_flow() {
    let invoices = vec![
        make_invoice("inv1", 500.0, InvoiceCategory::Hotel),
        make_invoice("inv2", 150.0, InvoiceCategory::Meal),
        make_city_transport_invoice("inv3", 100.0),
    ];

    let payments = vec![
        make_payment("pay1", 500.0, "测试酒店"),       // matches inv1
        make_payment("pay2", 150.0, "测试餐厅"),      // matches inv2
        make_payment("pay3", 30.0, "滴滴出行"),        // matches inv3 (one-to-many)
        make_payment("pay4", 40.0, "滴滴出行"),        // matches inv3 (one-to-many)
        make_payment("pay5", 30.0, "滴滴出行"),        // matches inv3 (one-to-many)
    ];

    let result = batch_match(&invoices, &payments, 1.0);

    // All should be matched
    assert_eq!(result.matched.len(), 3, "All 3 invoices should be matched");
    assert_eq!(result.unmatched_invoices.len(), 0, "No unmatched invoices");
    assert_eq!(result.unmatched_payments.len(), 0, "No unmatched payments");

    // Verify one-to-one matches
    let one_to_one: Vec<&MatchResult> = result.matched.iter()
        .filter(|r| matches!(r.match_type, MatchType::OneToOne))
        .collect();
    assert_eq!(one_to_one.len(), 2, "Should have 2 one-to-one matches");

    // Verify one-to-many match
    let one_to_many: Vec<&MatchResult> = result.matched.iter()
        .filter(|r| matches!(r.match_type, MatchType::OneToMany))
        .collect();
    assert_eq!(one_to_many.len(), 1, "Should have 1 one-to-many match");
    assert_eq!(one_to_many[0].invoice_id, "inv3");
}

/// Test that unmatched items are correctly identified.
#[test]
fn test_batch_match_with_unmatched() {
    let invoices = vec![
        make_invoice("inv1", 100.0, InvoiceCategory::Hotel),
        make_invoice("inv2", 999.0, InvoiceCategory::Meal),
    ];

    let payments = vec![
        make_payment("pay1", 100.0, "酒店"),   // matches inv1
        make_payment("pay2", 50.0, "超市"),    // no matching invoice
    ];

    let result = batch_match(&invoices, &payments, 1.0);

    assert_eq!(result.matched.len(), 1);
    assert_eq!(result.unmatched_invoices.len(), 1);
    assert_eq!(result.unmatched_invoices[0].id, "inv2");
    assert_eq!(result.unmatched_payments.len(), 1);
    assert_eq!(result.unmatched_payments[0].id, "pay2");
}

// ===== Form builder integration tests =====

/// Test the full flow from matching to form building.
#[test]
fn test_form_builder_integration() {
    let invoices = vec![
        make_invoice("inv1", 553.0, InvoiceCategory::Train),
        make_invoice("inv2", 1200.0, InvoiceCategory::Flight),
        make_invoice("inv3", 450.0, InvoiceCategory::Hotel),
        make_invoice("inv4", 80.0, InvoiceCategory::Meal),
    ];

    let payments = vec![
        make_payment("pay1", 553.0, "12306"),
        make_payment("pay2", 1200.0, "携程"),
        make_payment("pay3", 450.0, "酒店"),
        make_payment("pay4", 80.0, "餐厅"),
    ];

    // Step 1: Batch match
    let result = batch_match(&invoices, &payments, 1.0);
    assert_eq!(result.matched.len(), 4, "All invoices should be matched");

    // Step 2: Build reimbursement form
    let form = build_reimbursement_form(
        &result.matched,
        "张三",
        "技术部",
        "",
        "2025-01-15",
        "2025-01-20",
        2,
        "其他人员",
    );

    // Verify form fields
    assert_eq!(form.name, "张三");
    assert_eq!(form.department, "技术部");
    assert_eq!(form.travel_start, "2025-01-15");
    assert_eq!(form.travel_end, "2025-01-20");
    assert_eq!(form.companions, 2);

    // Verify category summaries
    assert_eq!(form.summaries.len(), 4, "Should have 4 category summaries");

    // Verify ordering: Train -> Flight -> Hotel -> Meal
    assert_eq!(form.summaries[0].category, InvoiceCategory::Train);
    assert_eq!(form.summaries[1].category, InvoiceCategory::Flight);
    assert_eq!(form.summaries[2].category, InvoiceCategory::Hotel);
    assert_eq!(form.summaries[3].category, InvoiceCategory::Meal);

    // Verify total amount (发票金额 + 伙食补助6天×100=600)
    let expected_total = 553.0 + 1200.0 + 450.0 + 80.0 + 600.0;
    println!("actual total: {:.2}, expected: {:.2}", form.total_amount, expected_total);
    println!("transport: {:.2}, city: {:.2}, hotel: {:.2}, meal_subsidy: {:.2}",
        form.transport_subtotal, form.city_transport_amount, form.hotel_subtotal, form.meal_subsidy.amount);
    assert!((form.total_amount - expected_total).abs() < 0.01,
        "Total amount should be {}", expected_total);
}

/// Test form builder with empty match results.
#[test]
fn test_form_builder_empty_results() {
    let form = build_reimbursement_form(
        &[],
        "李四",
        "市场部",
        "",
        "2025-02-01",
        "2025-02-05",
        0,
        "其他人员",
    );

    assert_eq!(form.name, "李四");
    assert!(form.summaries.is_empty());
    // 空匹配结果也包含伙食补助（5天×100=500）
    assert!((form.total_amount - 500.0).abs() < 0.01);
}

// ===== Manual match integration tests =====

/// Test manual match flow: create manual match, then unmatch.
#[test]
fn test_manual_match_and_unmatch_flow() {
    let invoice = make_invoice("inv1", 100.0, InvoiceCategory::Hotel);
    let payments = vec![
        make_payment("pay1", 60.0, "酒店"),
        make_payment("pay2", 40.0, "酒店"),
    ];

    // Step 1: Create manual match
    let result = create_manual_match(invoice.clone(), payments.clone());
    assert!(matches!(result.match_type, MatchType::ManualConfirmed));
    assert_eq!(result.invoice_id, "inv1");
    assert_eq!(result.payment_ids.len(), 2);
    // Total payment = 100, so confidence should be 1.0 (exact match)
    assert!((result.confidence - 1.0).abs() < f64::EPSILON);
    assert!((result.amount_diff).abs() < 0.01);

    // Step 2: Unmatch
    let (returned_invoice, returned_payments) = unmatch_invoice(&result);
    assert_eq!(returned_invoice.id, "inv1");
    assert_eq!(returned_payments.len(), 2);
}

/// Test manual match with amount mismatch.
#[test]
fn test_manual_match_amount_mismatch() {
    let invoice = make_invoice("inv1", 100.0, InvoiceCategory::Hotel);
    let payments = vec![
        make_payment("pay1", 70.0, "酒店"),
    ];

    let result = create_manual_match(invoice, payments);
    assert!(matches!(result.match_type, MatchType::ManualConfirmed));
    // Amount diff = 30, confidence should be 0.8 (not exact match)
    assert!((result.confidence - 0.8).abs() < f64::EPSILON);
    assert!((result.amount_diff - 30.0).abs() < 0.01);
}

// ===== End-to-end integration test =====

/// Full end-to-end flow: create data → batch match → build form → verify.
#[test]
fn test_end_to_end_flow() {
    // Step 1: Create realistic test data
    let invoices = vec![
        make_invoice("inv-train", 553.0, InvoiceCategory::Train),
        make_invoice("inv-flight", 1280.0, InvoiceCategory::Flight),
        make_invoice("inv-hotel1", 450.0, InvoiceCategory::Hotel),
        make_invoice("inv-hotel2", 450.0, InvoiceCategory::Hotel),
        make_invoice("inv-meal1", 120.0, InvoiceCategory::Meal),
        make_invoice("inv-meal2", 85.0, InvoiceCategory::Meal),
        make_city_transport_invoice("inv-taxi", 100.0),
    ];

    let payments = vec![
        make_payment("pay-train", 553.0, "12306"),
        make_payment("pay-flight", 1280.0, "携程机票"),
        make_payment("pay-hotel1", 450.0, "如家酒店"),
        make_payment("pay-hotel2", 450.0, "如家酒店"),
        make_payment("pay-meal1", 120.0, "全聚德"),
        make_payment("pay-meal2", 85.0, "肯德基"),
        make_payment("pay-taxi1", 30.0, "滴滴出行"),
        make_payment("pay-taxi2", 40.0, "滴滴出行"),
        make_payment("pay-taxi3", 30.0, "滴滴出行"),
    ];

    // Step 2: Batch match
    let match_result = batch_match(&invoices, &payments, 1.0);

    // All invoices should be matched
    assert_eq!(match_result.matched.len(), invoices.len(),
        "All invoices should be matched");
    assert_eq!(match_result.unmatched_invoices.len(), 0);
    assert_eq!(match_result.unmatched_payments.len(), 0);

    // Step 3: Build reimbursement form
    let form = build_reimbursement_form(
        &match_result.matched,
        "张三",
        "技术部",
        "",
        "2025-01-15",
        "2025-01-20",
        1,
        "其他人员",
    );

    // Step 4: Verify form
    assert_eq!(form.name, "张三");
    assert_eq!(form.department, "技术部");

    // Should have 5 categories: Train, Flight, Hotel, CityTransport, Meal
    assert_eq!(form.summaries.len(), 5);

    // Verify order and amounts
    assert_eq!(form.summaries[0].category, InvoiceCategory::Train);
    assert!((form.summaries[0].total_amount - 553.0).abs() < 0.01);
    assert_eq!(form.summaries[0].count, 1);

    assert_eq!(form.summaries[1].category, InvoiceCategory::Flight);
    assert!((form.summaries[1].total_amount - 1280.0).abs() < 0.01);

    assert_eq!(form.summaries[2].category, InvoiceCategory::CityTransport);
    assert!((form.summaries[2].total_amount - 100.0).abs() < 0.01);

    assert_eq!(form.summaries[3].category, InvoiceCategory::Hotel);
    assert!((form.summaries[3].total_amount - 900.0).abs() < 0.01);
    assert_eq!(form.summaries[3].count, 2);

    assert_eq!(form.summaries[4].category, InvoiceCategory::Meal);
    assert!((form.summaries[4].total_amount - 205.0).abs() < 0.01);
    assert_eq!(form.summaries[4].count, 2);

    // Total (发票金额 + 伙食补助6天×100=600)
    let expected_total = 553.0 + 1280.0 + 900.0 + 205.0 + 100.0 + 600.0;
    assert!((form.total_amount - expected_total).abs() < 0.01);
}
