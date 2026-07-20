#![cfg(feature = "pdfplumber")]

/// Visual verification of 4 PDFs against parser's claimed extraction.
/// Run with: cargo test --features pdfplumber --test visual_verify_pdfs -- --nocapture

use pdfplumber::{Pdf, TextOptions};
use std::path::Path;

fn extract_text(path: &Path) -> String {
    let pdf = Pdf::open_file(path, None).expect("Failed to open PDF");
    let mut text = String::new();
    for page_result in pdf.pages_iter() {
        if let Ok(page) = page_result {
            text.push_str(&page.extract_text(&TextOptions::default()));
            text.push('\n');
        }
    }
    text
}

fn extract_field(text: &str, patterns: &[&str]) -> Option<String> {
    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(text) {
                if caps.len() > 1 {
                    return Some(caps[1].trim().to_string());
                }
            }
        }
    }
    None
}

fn find_pdf(base: &str, substring: &str) -> std::path::PathBuf {
    let dir = Path::new("..").join("data").join(base);
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains(substring) {
            return entry.path();
        }
    }
    panic!("PDF not found: {} in {}", substring, dir.display());
}

#[test]
fn test_verify_all_pdfs() {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                    PDF VISUAL VERIFICATION REPORT                            ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
    
    let cases = vec![
        ("Didi Invoice A (市内交通)", "市内交通", "A.pdf", ParserClaim {
            seller: Some("成都滴滴优行科技有限公司"),
            amount: Some("523.57"),
            invoice_no: Some("26517000000358455168"),
            date: None,
            item: None,
        }),
        ("Hotel Invoice (住宿)", "住宿", "dzfp_26512000001996635841", ParserClaim {
            seller: Some("成都博朗君悦酒店管理有限责任公司"),
            amount: Some("2566.55"),
            invoice_no: Some("26512000001996635841"),
            date: Some("2026 年 05 月 14 日"),
            item: Some("住宿服务"),
        }),
        ("Flight Invoice (机票)", "机票", "9586482810622", ParserClaim {
            seller: Some("阿斯兰航空服务（上海）有限公司"),
            amount: Some("1740.00"),
            invoice_no: None,
            date: Some("2026-05-17"),
            item: None,
        }),
        ("Insurance Invoice (保险)", "保险", "20_电子发票", ParserClaim {
            seller: Some("众安在线财产保险股份有限公司"),
            amount: Some("50.00"),
            invoice_no: None,
            date: None,
            item: None,
        }),
    ];
    
    for (name, base, substring, claim) in cases {
        println!("\n┌──────────────────────────────────────────────────────────────────────────────┐");
        println!("│ PDF: {:<68} │", name);
        println!("└──────────────────────────────────────────────────────────────────────────────┘");
        
        let path = find_pdf(base, substring);
        println!("  File: {}", path.display());
        
        let text = extract_text(&path);
        
        println!("\n  RAW TEXT (first 800 chars):\n  {}", "─".repeat(70));
        for line in text.lines().take(20) {
            println!("    {}", line);
        }
        
        // Extract actual values
        let actual_seller = extract_field(&text, &[
            r"销售方 [^\n]*\n[^\n]*名称 [：:]\s*([^\n]+)",
            r"名称 [：:]\s*([^\n]+?)(?:\s* 统一社会信用代码)",
        ]);
        
        let actual_amount = extract_field(&text, &[
            r"价税合计 [^(]*（大写）[^(]*（小写）[^\d¥￥]*[¥￥]\s*([0-9,]+\.\d{2})",
            r"（小写）[^\d¥￥]*[¥￥]\s*([0-9,]+\.\d{2})",
        ]);
        
        let actual_invoice_no = extract_field(&text, &[
            r"发票号码 [：:]\s*([0-9]{16,})",
        ]);
        
        let actual_date = extract_field(&text, &[
            r"开票日期 [：:]\s*([0-9]{4}年 [0-9]{1,2}月 [0-9]{1,2}日)",
            r"([0-9]{4}-[0-9]{2}-[0-9]{2})",
        ]);
        
        let actual_item = extract_field(&text, &[
            r"\*([^\*]+)\*",
        ]);
        
        println!("\n  ┌─────────────────────────────────────────────────────────────────────────┐");
        println!("  │ FIELD           │ PARSER CLAIM        │ ACTUAL (from PDF)     │ MATCH │");
        println!("  ├─────────────────────────────────────────────────────────────────────────┤");
        
        let mut all_match = true;
        
        // Seller
        let seller_claim = claim.seller.unwrap_or("N/A");
        let seller_actual = actual_seller.as_deref().unwrap_or("NOT FOUND");
        let seller_match = if claim.seller.is_some() {
            actual_seller.as_deref() == claim.seller.as_deref().map(|s| s.trim())
        } else { true };
        if !seller_match { all_match = false; }
        println!("  │ Seller          │ {:<19} │ {:<21} │   {}   │", 
            seller_claim, seller_actual, if seller_match { "✓" } else { "✗" });
        
        // Amount
        let amount_claim = claim.amount.unwrap_or("N/A");
        let amount_actual = actual_amount.as_deref().unwrap_or("NOT FOUND");
        let amount_match = if claim.amount.is_some() {
            actual_amount.as_deref() == claim.amount.as_deref()
        } else { true };
        if !amount_match { all_match = false; }
        println!("  │ Amount          │ {:<19} │ {:<21} │   {}   │", 
            amount_claim, amount_actual, if amount_match { "✓" } else { "✗" });
        
        // Invoice No
        let inv_claim = claim.invoice_no.unwrap_or("N/A");
        let inv_actual = actual_invoice_no.as_deref().unwrap_or("NOT FOUND");
        let inv_match = if claim.invoice_no.is_some() {
            actual_invoice_no.as_deref() == claim.invoice_no.as_deref()
        } else { true };
        if !inv_match { all_match = false; }
        println!("  │ Invoice No      │ {:<19} │ {:<21} │   {}   │", 
            inv_claim, inv_actual, if inv_match { "✓" } else { "✗" });
        
        // Date
        let date_claim = claim.date.unwrap_or("N/A");
        let date_actual = actual_date.as_deref().unwrap_or("NOT FOUND");
        let date_match = if claim.date.is_some() {
            actual_date.as_deref() == claim.date.as_deref()
        } else { true };
        if !date_match { all_match = false; }
        println!("  │ Date            │ {:<19} │ {:<21} │   {}   │", 
            date_claim, date_actual, if date_match { "✓" } else { "✗" });
        
        // Item
        let item_claim = claim.item.unwrap_or("N/A");
        let item_actual = actual_item.as_deref().unwrap_or("NOT FOUND");
        let item_match = if claim.item.is_some() {
            actual_item.as_deref().map(|s| s.contains(claim.item.unwrap())) == Some(true)
        } else { true };
        if !item_match { all_match = false; }
        println!("  │ Item            │ {:<19} │ {:<21} │   {}   │", 
            item_claim, item_actual, if item_match { "✓" } else { "✗" });
        
        println!("  └─────────────────────────────────────────────────────────────────────────┘");
        
        if all_match {
            println!("  VERDICT: ✓ ALL FIELDS MATCH");
        } else {
            println!("  VERDICT: ✗ DISCREPANCIES FOUND");
        }
    }
    
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                           END OF VERIFICATION REPORT                         ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
}

#[derive(Clone)]
struct ParserClaim {
    seller: Option<&'static str>,
    amount: Option<&'static str>,
    invoice_no: Option<&'static str>,
    date: Option<&'static str>,
    item: Option<&'static str>,
}
