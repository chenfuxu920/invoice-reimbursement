use invoice_reimbursement_lib::models::invoice::{HotelDetail, Invoice, InvoiceCategory, InvoiceSource};
use invoice_reimbursement_lib::models::match_result::{MatchResult, MatchType};
use invoice_reimbursement_lib::models::payment::{PaymentRecord, PaymentSource};
use invoice_reimbursement_lib::pdf::form_builder::build_reimbursement_form;
use invoice_reimbursement_lib::pdf::form_html_generator::generate_reimbursement_html_string;
use chrono::NaiveDate;

fn make_hotel_match(
    id: &str,
    amount: f64,
    check_in: &str,
    check_out: &str,
    nights: usize,
    seller: &str,
) -> MatchResult {
    let check_in_date = NaiveDate::parse_from_str(check_in, "%Y-%m-%d").unwrap();
    let check_out_date = NaiveDate::parse_from_str(check_out, "%Y-%m-%d").unwrap();
    let invoice = Invoice {
        id: id.to_string(),
        invoice_number: format!("INV-{}", id),
        amount,
        seller_name: seller.to_string(),
        item_name: "*住宿服务*住宿费".to_string(),
        date: check_in_date,
        travel_date: None,
        category: InvoiceCategory::Hotel,
        source: InvoiceSource::Pdf("test.pdf".to_string()),
        itineraries: vec![],
        itinerary_file: None,
        remarks: format!(
            "{},订单日期:{}至{},共{}天,共1间",
            seller,
            &check_in[5..].replace('-', "-"),
            &check_out[5..].replace('-', "-"),
            nights
        ),
        hotel_detail: Some(HotelDetail {
            check_in: Some(check_in_date),
            check_out: Some(check_out_date),
            nights,
            nightly_rate: amount / nights as f64,
        }),
        departure_city: None,
        arrival_city: None,
                    toll_travel_time: None,
    };
    let payment = PaymentRecord {
        id: format!("pay-{}", id),
        transaction_id: format!("TX-{}", id),
        transaction_time: format!("{} 12:00", check_in),
        amount,
        discount: 0.0,
        merchant_name: seller.to_string(),
        source: PaymentSource::Wechat,
        category: "住宿".to_string(),
        payment_method: String::new(),
        original_amount: 0.0,
        refund_amount: 0.0,
    };
    MatchResult {
        invoice_id: id.to_string(),
        invoice,
        payment_ids: vec![payment.id.clone()],
        payments: vec![payment],
        match_type: MatchType::OneToOne,
        confidence: 1.0,
        amount_diff: 0.0,
        itinerary_payment_pairs: vec![],
    }
}

fn make_transport_match(id: &str, amount: f64, category: InvoiceCategory, seller: &str) -> MatchResult {
    let invoice = Invoice {
        id: id.to_string(),
        invoice_number: format!("INV-{}", id),
        amount,
        seller_name: seller.to_string(),
        item_name: "交通服务".to_string(),
        date: NaiveDate::from_ymd_opt(2025, 8, 4).unwrap(),
        travel_date: None,
        category,
        source: InvoiceSource::Pdf("test.pdf".to_string()),
        itineraries: vec![],
        itinerary_file: None,
        remarks: String::new(),
        hotel_detail: None,
        departure_city: None,
        arrival_city: None,
                    toll_travel_time: None,
    };
    let payment = PaymentRecord {
        id: format!("pay-{}", id),
        transaction_id: format!("TX-{}", id),
        transaction_time: "2025-08-04 12:00".to_string(),
        amount,
        discount: 0.0,
        merchant_name: seller.to_string(),
        source: PaymentSource::Wechat,
        category: "交通".to_string(),
        payment_method: String::new(),
        original_amount: 0.0,
        refund_amount: 0.0,
    };
    MatchResult {
        invoice_id: id.to_string(),
        invoice,
        payment_ids: vec![payment.id.clone()],
        payments: vec![payment],
        match_type: MatchType::OneToOne,
        confidence: 1.0,
        amount_diff: 0.0,
        itinerary_payment_pairs: vec![],
    }
}

#[test]
fn e2e_full_reimbursement_with_hotel_standard() {
    // 模拟一次完整出差的发票数据
    let match_results = vec![
        // 高铁票
        make_transport_match("inv-train", 553.0, InvoiceCategory::Train, "中国铁路"),
        // 飞机票
        make_transport_match("inv-flight", 1090.0, InvoiceCategory::Flight, "中国国航"),
        // 退改签
        make_transport_match("inv-change", 110.5, InvoiceCategory::TicketChange, "中国铁路"),
        // 市内交通
        make_transport_match("inv-city1", 500.0, InvoiceCategory::CityTransport, "滴滴出行"),
        make_transport_match("inv-city2", 456.65, InvoiceCategory::CityTransport, "高德打车"),
        // 住宿 - 成都（四川省标准 370元/晚，11晚）
        make_hotel_match(
            "inv-hotel",
            4222.63,  // 实际: 4222.63/11晚 ≈ 383.88/晚 > 标准 370/晚
            "2025-08-04",
            "2025-08-15",
            11,
            "成都景澜美居酒店",
        ),
    ];

    let form = build_reimbursement_form(
        &match_results,
        "陈福旭",
        "聘用工程师",
        "四川省成都市",
        "2025-08-04",
        "2025-08-15",
        0,
        "其他人员",
    );

    // 验证结构完整性（计算逻辑由 form_builder 单元测试覆盖）
    assert_eq!(form.name, "陈福旭");
    assert_eq!(form.department, "聘用工程师");
    assert_eq!(form.travel_days, 12);

    // 交通费
    assert_eq!(form.transport_details.len(), 3);
    assert!(form.transport_subtotal > 0.0);
    assert_eq!(form.city_transport_count, 2);
    assert!(form.city_transport_amount > 0.0);

    // 住宿费
    assert_eq!(form.hotel_levels.len(), 1);
    let hotel = &form.hotel_levels[0];
    assert_eq!(hotel.days, 11);
    assert!(hotel.actual_amount > 0.0);
    assert!(hotel.amount > 0.0);
    assert!(hotel.amount <= hotel.actual_amount + 0.01);

    // 伙食补助
    assert_eq!(form.meal_subsidy.days, 12);
    assert!(form.meal_subsidy.amount > 0.0);

    // 总额
    assert!(form.total_amount > 0.0);

    // HTML 包含关键内容
    let html = generate_reimbursement_html_string(&form);
    assert!(html.contains("差 旅 费 报 销 单"));
    assert!(html.contains("陈福旭"));
    assert!(html.contains("聘用工程师"));
    assert!(html.contains("四川省成都市"));
    assert!(html.contains(&format!("{:.2}", form.total_amount)));

    // 输出 HTML 文件供人工检查
    let output_dir = "../data";
    let output_path = format!("{}/报销单_E2E测试.html", output_dir);
    std::fs::write(&output_path, &html).expect("Failed to write HTML");
    println!("E2E 测试输出: {}", output_path);
    println!("住宿实际支付: {:.2}", hotel.actual_amount);
    println!("住宿可报销(封顶): {:.2}", hotel.amount);
    println!("住宿每晚标准: {:.2}", hotel.daily_rate);
    println!("总额: {:.2}", form.total_amount);
}
