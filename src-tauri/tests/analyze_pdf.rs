#![cfg(feature = "pdfplumber")]

use invoice_reimbursement_lib::pdf::debug_extract::debug_extract_texts;
use std::path::Path;

#[test]
fn analyze_quandian_invoice() {
    // The directory name shows as "未分类" but may have encoding issues in PowerShell output
    let pdf_path = r"C:\Projects\rust-projects\invoice-reimbursement\data\δ\ȫ緢Ʊ(ȫֻӷƱ).pdf";
    
    println!("\n=== PDF Analysis ===\n");
    println!("Looking for PDF at: {}", pdf_path);
    
    if !Path::new(pdf_path).exists() {
        println!("SKIP: PDF not found");
        // Try to find the file dynamically
        let data_dir = Path::new(r"C:\Projects\rust-projects\invoice-reimbursement\data");
        if let Ok(entries) = std::fs::read_dir(data_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                        for sub_entry in sub_entries.flatten() {
                            let name = sub_entry.file_name();
                            let name_str = name.to_string_lossy();
                            if name_str.contains("全电") || name_str.contains("数字化") {
                                println!("FOUND: {:?}", sub_entry.path());
                                analyze_file(&sub_entry.path());
                                return;
                            }
                        }
                    }
                }
            }
        }
        return;
    }
    
    analyze_file(Path::new(pdf_path));
}

fn analyze_file(pdf_path: &Path) {
    println!("\nAnalyzing: {:?}", pdf_path);
    
    match debug_extract_texts(pdf_path.to_str().unwrap(), 200, None) {
        Ok(result) => {
            println!("\n=== Results ===");
            println!("Pages: {}", result.pages.len());
            
            for (page_idx, page) in result.pages.iter().enumerate() {
                println!("\n--- Page {} ---", page_idx + 1);
                println!("Image size: {}x{}", page.width, page.height);
                println!("pdfplumber words: {}", page.pdfplumber.len());
                println!("OCR words: {}", page.ocr.len());
                
                if !page.pdfplumber.is_empty() {
                    println!("\nAll pdfplumber words:");
                    for (i, word) in page.pdfplumber.iter().enumerate() {
                        println!("  {}: '{}' @ ({:.1}, {:.1}) - {:.1}x{:.1}", 
                            i, word.text, word.x, word.y, word.w, word.h);
                    }
                    
                    println!("\nAll text (concatenated):");
                    let all_text: String = page.pdfplumber.iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("{}", all_text);
                }
                
                if !page.ocr.is_empty() {
                    println!("\nAll OCR words:");
                    for (i, word) in page.ocr.iter().take(30).enumerate() {
                        println!("  {}: '{}' @ ({:.1}, {:.1}) - {:.1}x{:.1} (conf: {:.0}%)", 
                            i, word.text, word.x, word.y, word.w, word.h, word.confidence);
                    }
                }
                
                println!("\nLogs:");
                for log in &result.logs.pdfplumber {
                    println!("  [pdfplumber] {}", log);
                }
            }
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}
