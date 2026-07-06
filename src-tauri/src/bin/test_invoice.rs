use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::pdf::invoice_pipeline::parse_invoice_from_pdf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: test_invoice <pdf_path>");
        std::process::exit(1);
    }
    let pdf_path = &args[1];

    for path in &["pdfium.dll".as_ref(), "..\\pdfium.dll".as_ref()] {
        if std::path::Path::new(path).exists() {
            let _ = invoice_reimbursement_lib::ocr::engine::init_pdfium(path);
            break;
        }
    }

    let mut engine = OcrEngine::new("models").expect("OCR init failed");
    match parse_invoice_from_pdf(pdf_path, &mut engine) {
        Ok(inv) => {
            println!("销售方: {}", inv.seller_name);
            println!("金额: {:.2}", inv.amount);
            println!("类别: {:?}", inv.category);
            println!("项目: {}", inv.item_name);
            println!("发票号: {}", inv.invoice_number);
        }
        Err(e) => println!("失败: {}", e),
    }
}
