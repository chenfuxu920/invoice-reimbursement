/// 调试：3个失败发票的文字提取诊断（不依赖 OCR）
/// 运行: cargo test --test debug_no_ocr -- --nocapture --ignored
use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::pdf::invoice_pipeline::parse_invoice_from_pdf;
use invoice_reimbursement_lib::pdf::text_extractor::{
    has_sufficient_text, is_garbled_items,
    extract_pdf_column_aware,
};

const FILES: &[&str] = &[
    "../data/③20260525-20260605_出差/原始文件/20_电子发票_20260604_225419_电子发票.pdf",
    "../data/③20260525-20260605_出差/原始文件/21_电子发票_20260604_225437_电子发票.pdf",
    "../data/原始发票/25_【票根】人工收费车道电子发票_20260607_202032_G6021430020_1cc32f6666ccbd8a96210a9dd37e521e.pdf",
];

#[test]
#[ignore]
fn debug_three_files_no_ocr() {
    for &path in FILES {
        println!("\n════════════════════════════════════════");
        println!("文件: {}", path);
        println!("════════════════════════════════════════");

        // 1. pdfplumber 列感知提取
        println!("\n--- Step 1: pdfplumber 列感知提取 ---");
        #[cfg(feature = "pdfplumber")]
        {
            match extract_pdf_column_aware(path) {
                Ok(extraction) => {
                    let items: Vec<_> = extraction.pages.iter().flat_map(|p| p.texts.clone()).collect();
                    println!("提取到 {} 个文本项, {} 个原始Word, sufficient(20)={}",
                        items.len(), extraction.raw_words.len(), has_sufficient_text(&items, 20));
                    println!("garbled(0.3)={}", is_garbled_items(&items, 0.3));
                    for (i, t) in items.iter().enumerate() {
                        println!("  [{}] '{}'", i, t.text);
                    }
                }
                Err(e) => println!("✗ pdfplumber 失败: {}", e),
            }
        }

        // 2. 完整 pipeline（用未初始化引擎 = 模拟无 OCR）
        println!("\n--- Step 2: 完整 pipeline (parse_invoice_from_pdf, 无OCR) ---");
        let mut engine = OcrEngine::uninitialized();
        match parse_invoice_from_pdf(path, &mut engine) {
            Ok(inv) => {
                println!("✓ pipeline 解析成功（无OCR）");
                println!("  invoice_number : '{}'", inv.invoice_number);
                println!("  amount         : {:.2}", inv.amount);
                println!("  seller_name    : '{}'", inv.seller_name);
                println!("  item_name      : '{}'", inv.item_name);
                println!("  date           : {}", inv.date);
                println!("  category       : {:?}", inv.category);
            }
            Err(e) => println!("✗ pipeline 解析失败: {}", e),
        }
    }
}
