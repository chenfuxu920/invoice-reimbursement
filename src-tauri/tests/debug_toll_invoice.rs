/// 调试测试：验证高速通行费发票能否被正确识别
/// 运行: cargo test --test debug_toll_invoice debug_toll_invoice_real -- --nocapture --ignored
use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::pdf::invoice_pipeline::parse_invoice_from_pdf;
use invoice_reimbursement_lib::parser::invoice_parser::parse_invoice_text;
use invoice_reimbursement_lib::models::invoice::InvoiceSource;

const MODELS_DIR: &str = "models";
const TOLL_PDF: &str = "../data/原始发票/25_【票根】人工收费车道电子发票_20260607_202032_G6021430020_1cc32f6666ccbd8a96210a9dd37e521e.pdf";

#[test]
#[ignore]
fn debug_toll_invoice_real() {
    println!("=== 高速通行费发票识别调试 ===");
    println!("文件: {}", TOLL_PDF);

    // 1. 用实际 pipeline 解析（含 OCR 回退）
    println!("\n--- Step 1: 完整 pipeline (parse_invoice_from_pdf) ---");
    let mut engine = match OcrEngine::new(MODELS_DIR) {
        Ok(e) => e,
        Err(e) => {
            println!("OCR 引擎初始化失败: {}", e);
            return;
        }
    };

    match parse_invoice_from_pdf(TOLL_PDF, &mut engine) {
        Ok(inv) => {
            println!("✓ pipeline 解析成功");
            print_invoice(&inv);
        }
        Err(e) => println!("✗ pipeline 解析失败: {}", e),
    }

    // 2. 单独看 OCR 原始输出（如果 pipeline 失败，这里帮助诊断）
    println!("\n--- Step 2: OCR 原始输出 ---");
    match engine.recognize_pdf(TOLL_PDF) {
        Ok(resp) => {
            println!("OCR 页数: {}", resp.pages.len());
            for (pi, page) in resp.pages.iter().enumerate() {
                println!("\n[Page {}] {} 条文本", pi, page.texts.len());
                for (i, t) in page.texts.iter().enumerate() {
                    let coords = t.box_coords.as_ref().map(|c| format!(" {}", c)).unwrap_or_default();
                    println!("  [{}] conf={:.2} '{}'{}", i, t.confidence, t.text, coords);
                }
            }

            // 3. 用 OCR 文本再跑一次 parse_invoice_text
            println!("\n--- Step 3: parse_invoice_text (OCR 文本) ---");
            let ocr_items: Vec<_> = resp.pages.iter().flat_map(|p| p.texts.clone()).collect();
            match parse_invoice_text(&ocr_items, InvoiceSource::Pdf("toll.pdf".to_string())) {
                Ok(inv) => print_invoice(&inv),
                Err(e) => println!("解析失败: {}", e),
            }
        }
        Err(e) => println!("OCR 失败: {}", e),
    }
}

fn print_invoice(inv: &invoice_reimbursement_lib::models::invoice::Invoice) {
    println!("  invoice_number : '{}'", inv.invoice_number);
    println!("  amount         : {:.2}", inv.amount);
    println!("  seller_name    : '{}'", inv.seller_name);
    println!("  item_name      : '{}'", inv.item_name);
    println!("  date           : {}", inv.date);
    println!("  category       : {:?}", inv.category);
    println!("  travel_date    : {:?}", inv.travel_date);
    println!("  toll_travel_time: {:?}", inv.toll_travel_time);
    println!("  departure_city : {:?}", inv.departure_city);
    println!("  arrival_city   : {:?}", inv.arrival_city);
    println!("  remarks        : '{}'", inv.remarks);
}
