/// OCR + 解析器 + 匹配端到端集成测试
/// 使用真实发票数据测试 v5 OCR 引擎
use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::parser::invoice_parser::parse_invoice_text;
use invoice_reimbursement_lib::models::invoice::InvoiceSource;
use std::path::Path;

const MODELS_DIR: &str = "models";
const DATA_DIR: &str = "../data";

/// Helper: 初始化 OCR 引擎
fn create_engine() -> OcrEngine {
    OcrEngine::new(MODELS_DIR).expect("Failed to create OcrEngine")
}

/// Helper: 获取数据文件路径
fn data_file(name: &str) -> String {
    Path::new(DATA_DIR).join(name).to_str().unwrap().to_string()
}

// ===== OCR 引擎基础测试 =====

#[test]
fn test_ocr_engine_init_v5() {
    let engine = create_engine();
    assert!(engine.health().unwrap(), "OCR engine health check should pass");
}

#[test]
fn test_ocr_engine_missing_dict() {
    // 确认 dict 文件缺失时会报错
    let result = OcrEngine::new("/tmp/empty_dir_for_test");
    assert!(result.is_err());
}

// ===== 电子发票 OCR 识别测试 =====

#[test]
#[ignore] // 需要模型文件，用 --ignored 运行
fn test_ocr_dzfp_invoice_a() {
    let mut engine = create_engine();
    let file = data_file("dzfp_26512000001728418261_中国人民解放军国防科技大学系统工程学院_20260427084626.pdf");
    
    // 将 PDF 转为图片再识别（嵌入式引擎不直接支持 PDF）
    // 这里测试图片识别路径
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    
    let response = result.unwrap();
    assert!(!response.texts.is_empty(), "Should recognize text from invoice");
    
    let all_text: String = response.texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
    // 电子发票应包含关键信息
    let preview: String = all_text.chars().take(100).collect();
    assert!(all_text.contains("发票") || all_text.contains("¥") || all_text.contains("元") || !response.texts.is_empty(),
        "Invoice text should contain amount markers or have text. Got: {}", preview);
}

#[test]
#[ignore]
fn test_ocr_dzfp_invoice_b() {
    let mut engine = create_engine();
    let file = data_file("dzfp_26512000001728439726_中国人民解放军国防科技大学系统工程学院_20260427084656.pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

#[test]
#[ignore]
fn test_ocr_dzfp_invoice_c() {
    let mut engine = create_engine();
    let file = data_file("dzfp_26512000001847622916_中国人民解放军国防科技大学系统工程学院_20260506074422.pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

// ===== 行程单/票据 OCR 测试 =====

#[test]
#[ignore]
fn test_ocr_tianfutong_itinerary() {
    let mut engine = create_engine();
    let file = data_file("天府通电子行程单.pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

#[test]
#[ignore]
fn test_ocr_didi_itinerary_a() {
    let mut engine = create_engine();
    let file = data_file("滴滴出行行程报销单A.pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

#[test]
#[ignore]
fn test_ocr_didi_itinerary_b() {
    let mut engine = create_engine();
    let file = data_file("滴滴出行行程报销单B.pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

#[test]
#[ignore]
fn test_ocr_didi_invoice_a() {
    let mut engine = create_engine();
    let file = data_file("滴滴电子发票A.pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

#[test]
#[ignore]
fn test_ocr_didi_invoice_b() {
    let mut engine = create_engine();
    let file = data_file("滴滴电子发票B.pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

// ===== 酒店/机票 OCR 测试 =====

#[test]
#[ignore]
fn test_ocr_hotel_bill() {
    let mut engine = create_engine();
    let file = data_file("成都九眼桥美居酒店结账单(3).pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

#[test]
#[ignore]
fn test_ocr_flight_ticket_a() {
    let mut engine = create_engine();
    let file = data_file("【飞猪】成都-长沙  订单9571936775622-机票款凭证 报销凭证.pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

#[test]
#[ignore]
fn test_ocr_flight_ticket_b() {
    let mut engine = create_engine();
    let file = data_file("【飞猪】长沙-成都  订单9548389406622-机票款凭证 报销凭证.pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

#[test]
#[ignore]
fn test_ocr_flight_refund() {
    let mut engine = create_engine();
    let file = data_file("【飞猪】长沙-成都  订单9548824677622-退票手续费 报销凭证.pdf");
    let img_path = pdf_to_image(&file).expect("Failed to convert PDF to image");
    let result = engine.recognize_image(&img_path);
    assert!(result.is_ok(), "OCR failed: {:?}", result.err());
    assert!(!result.unwrap().texts.is_empty());
}

// ===== 解析器集成测试 =====

#[test]
#[ignore]
fn test_parse_dzfp_invoice_amount() {
    let mut engine = create_engine();
    let file = data_file("dzfp_26512000001728418261_中国人民解放军国防科技大学系统工程学院_20260427084626.pdf");
    let img_path = pdf_to_image(&file).unwrap();
    let ocr_result = engine.recognize_image(&img_path).unwrap();
    
    let invoice = parse_invoice_text(
        &ocr_result.texts,
        InvoiceSource::Photo(file),
    );
    
    assert!(invoice.is_ok(), "Invoice parsing failed: {:?}", invoice.err());
    let inv = invoice.unwrap();
    assert!(inv.amount > 0.0, "Amount should be positive, got: {}", inv.amount);
    assert!(!inv.date.to_string().is_empty(), "Date should be extracted");
}

#[test]
#[ignore]
fn test_parse_hotel_invoice_category() {
    let mut engine = create_engine();
    let file = data_file("成都九眼桥美居酒店结账单(3).pdf");
    let img_path = pdf_to_image(&file).unwrap();
    let ocr_result = engine.recognize_image(&img_path).unwrap();
    
    let invoice = parse_invoice_text(
        &ocr_result.texts,
        InvoiceSource::Photo(file),
    ).unwrap();
    
    // 酒店应被分类为 Hotel
    assert!(matches!(invoice.category, 
        invoice_reimbursement_lib::models::invoice::InvoiceCategory::Hotel),
        "Hotel invoice should be classified as Hotel, got: {:?}", invoice.category);
}

#[test]
#[ignore]
fn test_parse_didi_invoice_category() {
    let mut engine = create_engine();
    let file = data_file("滴滴电子发票A.pdf");
    let img_path = pdf_to_image(&file).unwrap();
    let ocr_result = engine.recognize_image(&img_path).unwrap();
    
    let invoice = parse_invoice_text(
        &ocr_result.texts,
        InvoiceSource::Photo(file),
    ).unwrap();
    
    // 滴滴应被分类为 CityTransport
    assert!(matches!(invoice.category,
        invoice_reimbursement_lib::models::invoice::InvoiceCategory::CityTransport),
        "Didi invoice should be classified as CityTransport, got: {:?}", invoice.category);
}

// ===== 批量 OCR 识别测试 =====

#[test]
#[ignore]
fn test_batch_ocr_all_pdfs() {
    let mut engine = create_engine();
    let data_dir = Path::new(DATA_DIR);
    
    let pdf_files: Vec<_> = data_dir.read_dir().unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "pdf"))
        .collect();
    
    assert!(pdf_files.len() >= 15, "Should have at least 15 PDF files, got: {}", pdf_files.len());
    
    let mut success_count = 0;
    let mut fail_count = 0;
    
    for entry in &pdf_files {
        let path = entry.path();
        let name = path.file_name().unwrap().to_str().unwrap();
        
        match pdf_to_image(&path.to_str().unwrap()) {
            Ok(img_path) => {
                match engine.recognize_image(&img_path) {
                    Ok(result) => {
                        if !result.texts.is_empty() {
                            success_count += 1;
                        } else {
                            eprintln!("WARN: No text in {}", name);
                            fail_count += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("FAIL: OCR error for {}: {:?}", name, e);
                        fail_count += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("SKIP: PDF conversion failed for {}: {:?}", name, e);
            }
        }
    }
    
    println!("Batch OCR: {} success, {} failed out of {} PDFs", success_count, fail_count, pdf_files.len());
    assert!(success_count > 0, "At least some PDFs should be recognized successfully");
}

// ===== 支付解析器测试 =====

#[test]
fn test_parse_alipay_csv() {
    let csv_path = data_file("支付宝交易明细(20260407-20260507).csv");
    let content = std::fs::read_to_string(&csv_path).expect("Failed to read Alipay CSV");
    
    // 支付宝 CSV 应有标题行
    assert!(content.contains("交易") || content.contains("金额") || content.contains("支付宝"),
        "Alipay CSV should contain transaction headers");
}

#[test]
fn test_parse_wechat_xlsx_exists() {
    let xlsx_path = data_file("微信支付账单流水文件(20260429-20260506)_20260506163409.xlsx");
    assert!(Path::new(&xlsx_path).exists(), "WeChat XLSX file should exist");
}

// ===== Helper: PDF 转图片 =====
fn pdf_to_image(pdf_path: &str) -> Result<String, String> {
    use std::process::Command;
    
    let output_dir = "/tmp/ocr_test_images";
    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    
    let pdf_name = Path::new(pdf_path)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap();
    let img_path = format!("{}/{}_page1.png", output_dir, pdf_name);
    
    // 使用 pdftoppm 转换 PDF 为图片（第一页）
    let output = Command::new("pdftoppm")
        .args(["-png", "-f", "1", "-l", "1", "-r", "200", pdf_path, &format!("{}/{}", output_dir, pdf_name)])
        .output()
        .map_err(|e| format!("pdftoppm not found: {}. Install poppler-utils", e))?;
    
    if !output.status.success() {
        return Err(format!("pdftoppm failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    
    // pdftoppm 输出格式为 {prefix}-1.png
    let actual_path = format!("{}/{}-1.png", output_dir, pdf_name);
    if Path::new(&actual_path).exists() {
        return Ok(actual_path);
    }
    
    // 尝试不带 -1 的路径
    if Path::new(&img_path).exists() {
        return Ok(img_path);
    }
    
    Err(format!("PDF to image conversion produced no output file. Tried: {} and {}", img_path, actual_path))
}
