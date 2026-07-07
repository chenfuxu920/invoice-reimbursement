/// 调试：3个失败发票的文字提取诊断（不依赖 OCR）
/// 运行: cargo test --test debug_no_ocr -- --nocapture --ignored
use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::pdf::invoice_pipeline::parse_invoice_from_pdf;
use invoice_reimbursement_lib::pdf::text_extractor::{
    classify_pdf_document_type, extract_text_from_pdf, has_sufficient_text, is_garbled_items,
    extract_pdf_column_aware,
};
use invoice_reimbursement_lib::parser::invoice_parser::parse_invoice_text;
use invoice_reimbursement_lib::models::invoice::InvoiceSource;

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

        // 1. parangi 文字提取
        println!("\n--- Step 1: parangi 文字提取 ---");
        let parangi_items = match extract_text_from_pdf(path) {
            Ok(items) => {
                println!("提取到 {} 条文本, sufficient(20)={}", items.len(), has_sufficient_text(&items, 20));
                println!("garbled(0.3)={}", is_garbled_items(&items, 0.3));
                for (i, t) in items.iter().enumerate() {
                    println!("  [{}] '{}'", i, t.text);
                }
                items
            }
            Err(e) => {
                println!("✗ parangi 提取失败: {}", e);
                Vec::new()
            }
        };

        // 1b. pdf-extract 直接尝试（parangi 返回非空时不会走这个回退）
        println!("\n--- Step 1b: pdf-extract 直接提取 ---");
        let pdf_extract_result = std::panic::catch_unwind(|| {
            pdf_extract::extract_text(path)
        });
        match pdf_extract_result {
            Ok(Ok(text)) => {
                println!("pdf-extract 提取到 {} 字符", text.len());
                if text.len() > 20 {
                    println!("前 500 字符: {}", &text.chars().take(500).collect::<String>());
                } else {
                    println!("全文: '{}'", text);
                }
            }
            Ok(Err(e)) => println!("✗ pdf-extract 失败: {}", e),
            Err(_) => println!("✗ pdf-extract panic"),
        }

        // 2. pdfplumber 列感知提取
        println!("\n--- Step 2: pdfplumber 列感知提取 ---");
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

        // 3. 文档类型分类
        println!("\n--- Step 3: 文档类型分类 (parangi) ---");
        let doc_type = classify_pdf_document_type(&parangi_items);
        println!("分类结果: {:?}", doc_type);

        // 4. parse_invoice_text (parangi 文本)
        if !parangi_items.is_empty() {
            println!("\n--- Step 4: parse_invoice_text (parangi 文本) ---");
            match parse_invoice_text(&parangi_items, InvoiceSource::Pdf(path.to_string())) {
                Ok(inv) => {
                    println!("✓ 解析成功");
                    println!("  invoice_number : '{}'", inv.invoice_number);
                    println!("  amount         : {:.2}", inv.amount);
                    println!("  seller_name    : '{}'", inv.seller_name);
                    println!("  item_name      : '{}'", inv.item_name);
                    println!("  date           : {}", inv.date);
                    println!("  category       : {:?}", inv.category);
                    let seller_empty = inv.seller_name.is_empty();
                    let seller_garbled = inv.seller_name.contains("名称：")
                        || inv.seller_name.contains("名称:")
                        || inv.seller_name.contains('<')
                        || inv.seller_name.contains('>');
                    let number_missing = inv.invoice_number.is_empty();
                    println!("  → seller_empty={}, seller_garbled={}, number_missing={}",
                        seller_empty, seller_garbled, number_missing);
                    if seller_empty || seller_garbled || number_missing {
                        println!("  → ⚠ 将触发 OCR 回退（这就是失败的根因）");
                    }
                }
                Err(e) => {
                    println!("✗ 解析失败: {}", e);
                    println!("  → ⚠ 解析失败将触发 OCR 回退（这就是失败的根因）");
                }
            }
        }

        // 5. 完整 pipeline（用未初始化引擎 = 模拟无 OCR）
        println!("\n--- Step 5: 完整 pipeline (parse_invoice_from_pdf, 无OCR) ---");
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
