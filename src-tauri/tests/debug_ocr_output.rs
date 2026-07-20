/// 调试测试：输出 OCR 原始结果
use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::parser::invoice_parser::parse_invoice_text;
use invoice_reimbursement_lib::models::invoice::InvoiceSource;
use std::path::Path;

const MODELS_DIR: &str = "models";

fn pdf_to_image(pdf_path: &str) -> Result<String, String> {
    use std::process::Command;
    
    let output_dir = std::env::temp_dir().join("ocr_debug");
    std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    
    let pdf_name = Path::new(pdf_path).file_stem().unwrap().to_str().unwrap();
    
    let output = Command::new("pdftoppm")
        .args([
            "-png", "-f", "1", "-l", "1", "-r", "200",
            pdf_path,
            &output_dir.join(pdf_name).to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("pdftoppm error: {}", e))?;
    
    if !output.status.success() {
        return Err(format!("pdftoppm failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    
    let img_path = output_dir.join(format!("{}-1.png", pdf_name));
    if img_path.exists() {
        Ok(img_path.to_str().unwrap().to_string())
    } else {
        Err("Image not found".to_string())
    }
}

#[test]
#[ignore]
fn debug_dzfp_invoice() {
    let mut engine = OcrEngine::new(MODELS_DIR).expect("OCR init failed");
    let file = Path::new("../data/住宿")
        .join("dzfp_26512000001728418261_中国人民解放军国防科技大学系统工程学院_20260427084626.pdf")
        .to_str().unwrap().to_string();
    
    println!("Processing: {}", file);
    let img = pdf_to_image(&file).expect("PDF conversion failed");
    println!("Image: {}", img);
    
    let result = engine.recognize_image(&img).expect("OCR failed");
    
    println!("\n=== OCR Raw Output ===");
    println!("Total texts: {}", result.texts.len());
    
    for (i, text) in result.texts.iter().enumerate() {
        println!("[{}] conf={:.2} '{}'", i, text.confidence, text.text);
        if let Some(coords) = &text.box_coords {
            println!("    coords: {}", coords);
        }
    }
    
    println!("\n=== Parsing Attempt ===");
    match parse_invoice_text(&result.texts, InvoiceSource::Pdf("test.pdf".to_string())) {
        Ok(inv) => {
            println!("Success!");
            println!("  Amount: ¥{}", inv.amount);
            println!("  Seller: {}", inv.seller_name);
            println!("  Category: {:?}", inv.category);
        }
        Err(e) => println!("Failed: {}", e),
    }
}

#[test]
#[ignore]
fn debug_didi_invoice() {
    let mut engine = OcrEngine::new(MODELS_DIR).expect("OCR init failed");
    let file = "../data/市内交通/滴滴电子发票A.pdf".to_string();
    
    println!("Processing: {}", file);
    let img = pdf_to_image(&file).expect("PDF conversion failed");
    
    let result = engine.recognize_image(&img).expect("OCR failed");
    
    println!("\n=== OCR Raw Output ===");
    println!("Total texts: {}", result.texts.len());
    
    for (i, text) in result.texts.iter().enumerate() {
        println!("[{}] conf={:.2} '{}'", i, text.confidence, text.text);
    }
    
    println!("\n=== Parsing Attempt ===");
    match parse_invoice_text(&result.texts, InvoiceSource::Pdf("test.pdf".to_string())) {
        Ok(inv) => {
            println!("Success!");
            println!("  Amount: ¥{}", inv.amount);
            println!("  Seller: {}", inv.seller_name);
            println!("  Category: {:?}", inv.category);
        }
        Err(e) => println!("Failed: {}", e),
    }
}

#[test]
#[ignore]
fn debug_hotel_invoice() {
    let mut engine = OcrEngine::new(MODELS_DIR).expect("OCR init failed");
    let file = "../data/未分类/成都九眼桥美居酒店结账单(3).pdf".to_string();
    
    println!("Processing: {}", file);
    let img = pdf_to_image(&file).expect("PDF conversion failed");
    
    let result = engine.recognize_image(&img).expect("OCR failed");
    
    println!("\n=== OCR Raw Output ===");
    println!("Total texts: {}", result.texts.len());
    
    for (i, text) in result.texts.iter().enumerate() {
        println!("[{}] conf={:.2} '{}'", i, text.confidence, text.text);
    }
    
    println!("\n=== Parsing Attempt ===");
    match parse_invoice_text(&result.texts, InvoiceSource::Pdf("test.pdf".to_string())) {
        Ok(inv) => {
            println!("Success!");
            println!("  Amount: ¥{}", inv.amount);
            println!("  Seller: {}", inv.seller_name);
            println!("  Category: {:?}", inv.category);
        }
        Err(e) => println!("Failed: {}", e),
    }
}

#[test]
#[ignore]
fn debug_flight_invoice() {
    let mut engine = OcrEngine::new(MODELS_DIR).expect("OCR init failed");
    let file = "../data/机票/【飞猪】成都-长沙  订单9571936775622-机票款凭证 报销凭证.pdf".to_string();
    
    println!("Processing: {}", file);
    let img = pdf_to_image(&file).expect("PDF conversion failed");
    
    let result = engine.recognize_image(&img).expect("OCR failed");
    
    println!("\n=== OCR Raw Output ===");
    println!("Total texts: {}", result.texts.len());
    
    for (i, text) in result.texts.iter().enumerate() {
        println!("[{}] conf={:.2} '{}'", i, text.confidence, text.text);
    }
    
    println!("\n=== Parsing Attempt ===");
    match parse_invoice_text(&result.texts, InvoiceSource::Pdf("test.pdf".to_string())) {
        Ok(inv) => {
            println!("Success!");
            println!("  Amount: ¥{}", inv.amount);
            println!("  Seller: {}", inv.seller_name);
            println!("  Category: {:?}", inv.category);
        }
        Err(e) => println!("Failed: {}", e),
    }
}
