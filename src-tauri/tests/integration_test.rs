use invoice_reimbursement_lib::{
    models::invoice::{InvoiceCategory, InvoiceSource},
    ocr::structured_output::{BoundingBox, OcrStructuredOutput, OcrTextBlock, PageLayout, TextBlockType},
    parser::invoice_parser::parse_structured_invoice,
    parser::invoice_type_detector::{InvoiceType, InvoiceTypeDetector},
};

fn create_ocr_output_from_texts(texts: Vec<&str>) -> OcrStructuredOutput {
    let blocks = texts
        .iter()
        .enumerate()
        .map(|(i, text)| OcrTextBlock {
            text: text.to_string(),
            confidence: 0.95,
            bbox: BoundingBox {
                x: 0.0,
                y: (i * 30) as f64,
                width: 400.0,
                height: 25.0,
            },
            line_index: i,
            block_type: if text.contains("：") || text.contains(":") {
                TextBlockType::KeyValue
            } else {
                TextBlockType::Other
            },
        })
        .collect();

    OcrStructuredOutput {
        blocks,
        layout: PageLayout {
            width: 600.0,
            height: 1000.0,
            text_regions: vec![],
        },
    }
}

#[test]
fn test_vat_invoice_complete_parsing() {
    let ocr = create_ocr_output_from_texts(vec![
        "增值税电子普通发票",
        "发票号码：26512000001728418261",
        "开票日期：2025年03月15日",
        "销售方信息",
        "名称：四川景澜酒店管理有限公司",
        "纳税人识别号：91510100MA6C",
        "货物或应税劳务、服务名称",
        "*住宿服务*住宿费",
        "金额：¥1045.24",
        "价税合计：¥1045.24",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("test_vat.pdf".to_string()));
    assert!(result.is_ok(), "VAT invoice parsing should succeed");

    let invoice = result.unwrap();

    assert!(invoice.amount > 0.0, "Amount should be positive");
    assert!(!invoice.invoice_number.is_empty(), "Invoice number should not be empty");
    assert!(
        !matches!(invoice.category, InvoiceCategory::Other),
        "Category should not be Other, got {:?}",
        invoice.category
    );

    assert!((invoice.amount - 1045.24).abs() < 0.01);
    assert_eq!(invoice.invoice_number, "26512000001728418261");
    assert_eq!(invoice.category, InvoiceCategory::Hotel);
}

#[test]
fn test_didi_invoice_parsing() {
    let ocr = create_ocr_output_from_texts(vec![
        "滴滴出行电子发票",
        "发票号码：12345678",
        "开票日期：2025年02月20日",
        "销售方：滴滴出行科技有限公司",
        "项目名称：*运输服务*网约车服务费",
        "金额：¥35.50",
        "价税合计：¥35.50",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("didi_invoice.pdf".to_string()));
    assert!(result.is_ok(), "Didi invoice parsing should succeed");

    let invoice = result.unwrap();

    assert!(invoice.amount > 0.0, "Amount should be positive");
    assert!(!invoice.invoice_number.is_empty(), "Invoice number should not be empty");
    assert!(
        !matches!(invoice.category, InvoiceCategory::Other),
        "Category should not be Other, got {:?}",
        invoice.category
    );

    assert!((invoice.amount - 35.50).abs() < 0.01);
    assert_eq!(invoice.invoice_number, "12345678");
    assert_eq!(invoice.category, InvoiceCategory::CityTransport);
}

#[test]
fn test_didi_trip_parsing() {
    let ocr = create_ocr_output_from_texts(vec![
        "滴滴出行行程单",
        "行程时间：2025-01-15 09:30:00",
        "出发地：北京站",
        "目的地：国贸",
        "金额：¥28.00",
        "合计金额：¥28.00",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("didi_trip.pdf".to_string()));
    assert!(result.is_ok(), "Didi trip parsing should succeed");

    let invoice = result.unwrap();

    assert!(invoice.amount > 0.0, "Amount should be positive");
    assert_eq!(invoice.category, InvoiceCategory::CityTransport);
    assert!((invoice.amount - 28.00).abs() < 0.01);
}

#[test]
fn test_flight_invoice_parsing() {
    let ocr = create_ocr_output_from_texts(vec![
        "航空运输电子客票行程单",
        "旅客姓名：张三",
        "航班号：CA1234",
        "出发城市：成都",
        "到达城市：长沙",
        "日期：2025年05月10日",
        "价税合计：¥680.00",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("flight.pdf".to_string()));
    assert!(result.is_ok(), "Flight invoice parsing should succeed");

    let invoice = result.unwrap();

    assert!(invoice.amount > 0.0, "Amount should be positive");
    assert!(
        !matches!(invoice.category, InvoiceCategory::Other),
        "Category should not be Other, got {:?}",
        invoice.category
    );

    assert_eq!(invoice.category, InvoiceCategory::Flight);
    assert!((invoice.amount - 680.00).abs() < 0.01);
}

#[test]
fn test_train_invoice_parsing() {
    let ocr = create_ocr_output_from_texts(vec![
        "铁路电子客票",
        "发票号码：E123456789",
        "出发站：北京南站",
        "到达站：上海虹桥站",
        "车次：G1",
        "日期：2025年04月15日",
        "金额：¥553.00",
        "合计金额：¥553.00",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("train.pdf".to_string()));
    assert!(result.is_ok(), "Train invoice parsing should succeed");

    let invoice = result.unwrap();

    assert!(invoice.amount > 0.0, "Amount should be positive");
    assert_eq!(invoice.category, InvoiceCategory::Train);
    assert!((invoice.amount - 553.00).abs() < 0.01);
}

#[test]
fn test_hotel_invoice_with_tax_code() {
    let ocr = create_ocr_output_from_texts(vec![
        "增值税电子普通发票",
        "*住宿服务*住宿费",
        "价税合计：¥800.00",
        "开票方：如家酒店",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("hotel.pdf".to_string()));
    assert!(result.is_ok(), "Hotel invoice parsing should succeed");

    let invoice = result.unwrap();
    assert_eq!(invoice.category, InvoiceCategory::Hotel);
    assert!((invoice.amount - 800.00).abs() < 0.01);
}

#[test]
fn test_multiple_amount_extraction_strategy() {
    let ocr = create_ocr_output_from_texts(vec![
        "滴滴出行发票",
        "消费金额 ￥25.00",
        "优惠券 ￥-5.00",
        "实付金额 ￥20.00",
        "价税合计：¥20.00",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("multi_amount.pdf".to_string()));
    assert!(result.is_ok(), "Multiple amount parsing should succeed");

    let invoice = result.unwrap();
    assert!(invoice.amount > 0.0, "Amount should be positive");
    assert_eq!(invoice.category, InvoiceCategory::CityTransport);
}

#[test]
fn test_invoice_type_detection_vat() {
    let ocr = create_ocr_output_from_texts(vec![
        "增值税电子发票",
        "发票号码：12345678",
        "金额：¥100.00",
    ]);

    let invoice_type = InvoiceTypeDetector::detect(&ocr);
    assert!(matches!(invoice_type, InvoiceType::VatElectronicInvoice));
}

#[test]
fn test_invoice_type_detection_flight() {
    let ocr = create_ocr_output_from_texts(vec![
        "航空运输电子客票行程单",
        "航班号：CA1234",
        "票价：¥500.00",
    ]);

    let invoice_type = InvoiceTypeDetector::detect(&ocr);
    assert!(matches!(invoice_type, InvoiceType::FlightInvoice));
}

#[test]
fn test_invoice_type_detection_didi_invoice() {
    let ocr = create_ocr_output_from_texts(vec![
        "滴滴出行电子发票",
        "网约车服务费",
        "金额：¥35.00",
    ]);

    let invoice_type = InvoiceTypeDetector::detect(&ocr);
    assert!(matches!(invoice_type, InvoiceType::RideHailingInvoice));
}

#[test]
fn test_invoice_type_detection_didi_trip() {
    let ocr = create_ocr_output_from_texts(vec![
        "滴滴出行行程单",
        "行程时间：2025-01-15",
        "金额：¥28.00",
    ]);

    let invoice_type = InvoiceTypeDetector::detect(&ocr);
    assert!(matches!(invoice_type, InvoiceType::RideHailingItinerary));
}

#[test]
fn test_full_text_classification_accuracy() {
    let test_cases = vec![
        ("*住宿服务*住宿费", InvoiceCategory::Hotel),
        ("*运输服务*网约车", InvoiceCategory::CityTransport),
        ("*客运服务*出租", InvoiceCategory::CityTransport),
        ("*航空运输服务*机票", InvoiceCategory::Flight),
        ("酒店住宿费", InvoiceCategory::Hotel),
        ("滴滴出行", InvoiceCategory::CityTransport),
        ("航空公司机票", InvoiceCategory::Flight),
        ("铁路车票", InvoiceCategory::Train),
        ("餐饮服务", InvoiceCategory::Meal),
        ("退票费", InvoiceCategory::TicketChange),
    ];

    for (text, expected_category) in test_cases {
        let ocr = create_ocr_output_from_texts(vec![text, "金额：¥100.00"]);
        let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("test.pdf".to_string()));
        
        assert!(result.is_ok(), "Parsing should succeed for '{}'", text);
        let invoice = result.unwrap();
        
        assert_eq!(
            invoice.category, expected_category,
            "Classification for '{}' should be {:?}, got {:?}",
            text, expected_category, invoice.category
        );
    }
}

#[test]
fn test_field_extraction_strategies() {
    let ocr = create_ocr_output_from_texts(vec![
        "销售方信息",
        "名称：测试公司",
        "项目名称：交通服务费",
        "发票号码：87654321",
        "开票日期：2025年05月08日",
        "价税合计：¥150.00",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("test.pdf".to_string()));
    assert!(result.is_ok());

    let invoice = result.unwrap();

    assert!(!invoice.invoice_number.is_empty(), "Invoice number should be extracted");
    assert!(invoice.amount > 0.0, "Amount should be extracted");
    assert_eq!(invoice.invoice_number, "87654321");
    assert!((invoice.amount - 150.00).abs() < 0.01);
}

#[test]
fn test_invoice_with_no_invoice_number() {
    let ocr = create_ocr_output_from_texts(vec![
        "餐饮发票",
        "金额：¥100.00",
        "销售方：测试餐厅",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("no_number.pdf".to_string()));
    assert!(result.is_ok(), "Parsing should succeed even without invoice number");

    let invoice = result.unwrap();
    assert!(invoice.invoice_number.is_empty(), "Invoice number should be empty");
    assert_eq!(invoice.category, InvoiceCategory::Meal);
}

#[test]
fn test_invoice_with_large_amount() {
    let ocr = create_ocr_output_from_texts(vec![
        "酒店发票",
        "价税合计：¥12,345.67",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("large.pdf".to_string()));
    assert!(result.is_ok());

    let invoice = result.unwrap();
    assert!((invoice.amount - 12345.67).abs() < 0.01);
    assert_eq!(invoice.category, InvoiceCategory::Hotel);
}

#[test]
fn test_mixed_chinese_english_punctuation() {
    let ocr = create_ocr_output_from_texts(vec![
        "发票号码: 12345678",
        "金额: ¥200.00",
        "开票日期: 2025年05月08日",
    ]);

    let result = parse_structured_invoice(&ocr, InvoiceSource::Pdf("mixed.pdf".to_string()));
    assert!(result.is_ok());

    let invoice = result.unwrap();
    assert_eq!(invoice.invoice_number, "12345678");
    assert!((invoice.amount - 200.00).abs() < 0.01);
}

#[test]
#[ignore]
fn test_real_invoice_files_if_exist() {
    let test_files = vec![
        ("data/发票与行程单/滴滴电子发票A.pdf", InvoiceCategory::CityTransport),
    ];

    for (file_path, _expected_category) in test_files {
        if !std::path::Path::new(file_path).exists() {
            eprintln!("Skipping {} - file not found", file_path);
            continue;
        }

        println!("Testing real invoice: {}", file_path);
    }
}
