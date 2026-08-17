use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::pdf::invoice_pipeline::parse_itinerary_from_pdf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: test_itinerary <pdf_path>");
        std::process::exit(1);
    }
    let pdf_path = &args[1];
    let mut engine = OcrEngine::new("models").expect("OCR init failed");
    match parse_itinerary_from_pdf(pdf_path, &mut engine) {
        Ok(doc) => {
            println!("文件: {}", doc.file_name);
            println!("行程数: {}", doc.itineraries.len());
            println!("合计: {:.2}", doc.total_amount);
            for it in &doc.itineraries {
                println!(
                    "  {} | {} | {} → {} | ¥{:.2}",
                    it.date_time, it.provider, it.pickup, it.dropoff, it.amount
                );
            }
        }
        Err(e) => println!("失败: {}", e),
    }
}
