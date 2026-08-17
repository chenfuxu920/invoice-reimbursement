use chrono::NaiveDate;
/// PDF 生成集成测试
/// 测试报销表单 PDF 和发票-支付对照 PDF 的实际生成
use invoice_reimbursement_lib::models::invoice::{
    Invoice, InvoiceCategory, InvoiceSource, Itinerary,
};
use invoice_reimbursement_lib::models::match_result::{MatchResult, MatchType};
use invoice_reimbursement_lib::models::payment::{PaymentRecord, PaymentSource};
use invoice_reimbursement_lib::models::reimbursement::{
    CategorySummary, MealSubsidyDetail, ReimbursementForm, TransportDetail,
};
use invoice_reimbursement_lib::pdf::comparison_generator::generate_comparison_pdf;
use invoice_reimbursement_lib::pdf::form_generator::generate_reimbursement_pdf;
use std::path::Path;

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
        toll_travel_time: None,
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

#[test]
fn test_generate_reimbursement_pdf() {
    let form = ReimbursementForm {
        name: "张三".to_string(),
        department: "技术部".to_string(),
        destination: "北京".to_string(),
        travel_start: "2025-01-10".to_string(),
        travel_end: "2025-01-15".to_string(),
        travel_days: 6,
        companions: 1,
        transport_details: vec![
            TransportDetail {
                label: "车、船票".to_string(),
                count: 2,
                amount: 1106.0,
            },
            TransportDetail {
                label: "飞机票".to_string(),
                count: 1,
                amount: 1200.0,
            },
        ],
        transport_subtotal: 2306.0,
        city_transport_count: 5,
        city_transport_amount: 200.0,
        city_transport_actual_amount: 200.0,
        hotel_levels: vec![],
        hotel_subtotal: 1500.0,
        meal_subsidy: MealSubsidyDetail {
            persons: 1,
            days: 6,
            daily_rate: 100.0,
            amount: 600.0,
        },
        baggage_amount: 0.0,
        meal_reimbursement: 0.0,
        summaries: vec![
            CategorySummary {
                category: InvoiceCategory::Train,
                count: 2,
                total_amount: 1106.0,
            },
            CategorySummary {
                category: InvoiceCategory::Flight,
                count: 1,
                total_amount: 1200.0,
            },
            CategorySummary {
                category: InvoiceCategory::Hotel,
                count: 3,
                total_amount: 1500.0,
            },
            CategorySummary {
                category: InvoiceCategory::CityTransport,
                count: 5,
                total_amount: 200.0,
            },
        ],
        total_amount: 4606.0,
    };

    let output_path = "/tmp/test_reimbursement_form.pdf";
    let result = generate_reimbursement_pdf(&form, output_path);

    assert!(result.is_ok(), "PDF generation failed: {:?}", result.err());
    assert!(Path::new(output_path).exists(), "PDF file should exist");

    let metadata = std::fs::metadata(output_path).unwrap();
    assert!(metadata.len() > 0, "PDF file should not be empty");
    assert!(
        metadata.len() > 1000,
        "PDF file should have reasonable size (>1KB)"
    );

    // 清理
    std::fs::remove_file(output_path).ok();
}

#[test]
fn test_generate_comparison_pdf() {
    let match_results = vec![
        MatchResult {
            invoice_id: "inv1".to_string(),
            invoice: make_invoice("inv1", 553.0, InvoiceCategory::Train),
            payment_ids: vec!["pay1".to_string()],
            payments: vec![make_payment("pay1", 553.0, "12306")],
            match_type: MatchType::OneToOne,
            confidence: 1.0,
            amount_diff: 0.0,
            itinerary_payment_pairs: vec![],
            shared_payment_ids: vec![],
            shared_from_invoice_id: None,
        },
        MatchResult {
            invoice_id: "inv2".to_string(),
            invoice: make_invoice("inv2", 1200.0, InvoiceCategory::Flight),
            payment_ids: vec!["pay2".to_string()],
            payments: vec![make_payment("pay2", 1200.0, "携程")],
            match_type: MatchType::OneToOne,
            confidence: 0.95,
            amount_diff: 0.0,
            itinerary_payment_pairs: vec![],
            shared_payment_ids: vec![],
            shared_from_invoice_id: None,
        },
    ];

    let output_path = "/tmp/test_comparison.pdf";
    let result = generate_comparison_pdf(
        &match_results,
        &["inv3".to_string()], // 未匹配发票
        &["pay3".to_string()], // 未匹配支付
        output_path,
    );

    assert!(
        result.is_ok(),
        "Comparison PDF generation failed: {:?}",
        result.err()
    );
    assert!(
        Path::new(output_path).exists(),
        "Comparison PDF file should exist"
    );

    let metadata = std::fs::metadata(output_path).unwrap();
    assert!(
        metadata.len() > 0,
        "Comparison PDF file should not be empty"
    );

    // 清理
    std::fs::remove_file(output_path).ok();
}

#[test]
fn test_generate_empty_reimbursement_pdf() {
    let form = ReimbursementForm {
        name: "李四".to_string(),
        department: "财务部".to_string(),
        destination: String::new(),
        travel_start: "2025-02-01".to_string(),
        travel_end: "2025-02-05".to_string(),
        travel_days: 5,
        companions: 0,
        transport_details: vec![],
        transport_subtotal: 0.0,
        city_transport_count: 0,
        city_transport_amount: 0.0,
        city_transport_actual_amount: 0.0,
        hotel_levels: vec![],
        hotel_subtotal: 0.0,
        meal_subsidy: MealSubsidyDetail {
            persons: 0,
            days: 0,
            daily_rate: 100.0,
            amount: 0.0,
        },
        baggage_amount: 0.0,
        meal_reimbursement: 0.0,
        summaries: vec![],
        total_amount: 0.0,
    };

    let output_path = "/tmp/test_empty_reimbursement.pdf";
    let result = generate_reimbursement_pdf(&form, output_path);

    assert!(
        result.is_ok(),
        "Empty PDF generation failed: {:?}",
        result.err()
    );
    assert!(
        Path::new(output_path).exists(),
        "Empty PDF file should exist"
    );

    // 清理
    std::fs::remove_file(output_path).ok();
}
