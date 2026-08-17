use invoice_reimbursement_lib::{
    ocr::structured_output::{
        BoundingBox, OcrStructuredOutput, OcrTextBlock, PageLayout, TextBlockType,
    },
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
    let ocr =
        create_ocr_output_from_texts(vec!["滴滴出行电子发票", "网约车服务费", "金额：¥35.00"]);

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
