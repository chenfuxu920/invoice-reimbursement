use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::pdf::text_extractor::extract_text_from_pdf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: dump_ocr <pdf_path>");
        std::process::exit(1);
    }
    let pdf_path = &args[1];

    println!("=== parangi 纯文本 ===");
    match extract_text_from_pdf(pdf_path) {
        Ok(items) => {
            for (i, item) in items.iter().enumerate() {
                println!("  [{}] {}", i, item.text);
            }
        }
        Err(e) => println!("失败: {}", e),
    }

    let mut engine = OcrEngine::new("models").expect("OCR init failed");
    match engine.recognize_pdf(pdf_path) {
        Ok(resp) => {
            for (pi, page) in resp.pages.iter().enumerate() {
                println!("\nPage {} ({} blocks):", pi + 1, page.texts.len());
                for (i, item) in page.texts.iter().enumerate() {
                    let xy = item.box_coords.as_ref()
                        .and_then(|v| {
                            let pts = v.get("points")?.as_array()?;
                            let xs: Vec<f64> = pts.iter().filter_map(|p| p.get("x")?.as_f64()).collect();
                            let ys: Vec<f64> = pts.iter().filter_map(|p| p.get("y")?.as_f64()).collect();
                            if xs.is_empty() || ys.is_empty() { return None; }
                            let cx = (xs.iter().cloned().fold(f64::INFINITY, f64::min) + xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) / 2.0;
                            let cy = (ys.iter().cloned().fold(f64::INFINITY, f64::min) + ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) / 2.0;
                            Some(format!("({:.0},{:.0})", cx, cy))
                        })
                        .unwrap_or_else(|| "(-,-)".to_string());
                    println!("  [{}] {} {} conf:{:.2}", i, xy, item.text, item.confidence);
                }
            }
        }
        Err(e) => println!("OCR 失败: {}", e),
    }
}
