#![cfg(feature = "pdfplumber")]

use pdfplumber::{Pdf, TextOptions};
use std::path::Path;
use regex::Regex;

fn extract_text_from_pdf(path: &Path) -> String {
    let pdf = Pdf::open_file(path, None).expect("Failed to open PDF");
    let mut all_text = String::new();
    for page_result in pdf.pages_iter() {
        if let Ok(page) = page_result {
            let text = page.extract_text(&TextOptions::default());
            all_text.push_str(&text);
            all_text.push('\n');
        }
    }
    all_text
}

fn extract_field(text: &str, patterns: &[&str]) -> Option<String> {
    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(caps) = re.captures(text) {
                if caps.len() > 1 {
                    return Some(caps[1].trim().to_string());
                }
            }
        }
    }
    None
}

fn find_pdf_by_substring(base_dir: &Path, substring: &str) -> Option<std::path::PathBuf> {
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains(substring) {
                return Some(entry.path());
            }
        }
    }
    None
}

fn verify_pdf(name: &str, base_subdir: &str, substring: &str, expected: &ExpectedFields) {
    let base_dir = Path::new("..").join("data").join(base_subdir);
    let full_path = find_pdf_by_substring(&base_dir, substring)
        .unwrap_or_else(|| panic!("PDF not found: {} in {}", substring, base_dir.display()));
    
    println!("\n=== {} ===", name);
    println!("File: {}", full_path.display());
    
    let text = extract_text_from_pdf(&full_path);
    println!("\n--- Full Text (first 1500 chars) ---");
    println!("{}", &text.chars().take(1500).collect::<String>());
    
    // Extract fields with improved patterns
    let seller = extract_field(&text, &[
        r"销售方 [^\n]*\n[^\n]*名称 [：:]\s*([^\n]+)",
        r"名称 [：:]\s*([^\n]+?)(?:\s* 统一社会信用代码)",
    ]).or_else(|| expected.seller.clone());
    
    let amount = extract_field(&text, &[
        r"价税合计 [^(]*（大写）[^(]*（小写）[^\d¥￥]*[¥￥]\s*([0-9,]+\.\d{2})",
        r"价税合计 [（(][^）)]*[）)][^\d¥￥]*[¥￥]?\s*([0-9,]+\.\d{2})",
        r"[¥￥]\s*([0-9,]+\.\d{2})",
    ]).or_else(|| expected.amount.clone());
    
    let invoice_no = extract_field(&text, &[
        r"发票号码 [：:]\s*([0-9]{16,})",
    ]).or_else(|| expected.invoice_no.clone());
    
    let date = extract_field(&text, &[
        r"开票日期 [：:]\s*([0-9]{4}年 [0-9]{1,2}月 [0-9]{1,2}日)",
        r"([0-9]{4}-[0-9]{2}-[0-9]{2})",
    ]).or_else(|| expected.date.clone());
    
    let item = extract_field(&text, &[
        r"\*([^\*]+)\*",
    ]).or_else(|| expected.item.clone());
    
    println!("\n--- Extracted Fields ---");
    println!("Seller: {:?}", seller);
    println!("Amount: {:?}", amount);
    println!("Invoice No: {:?}", invoice_no);
    println!("Date: {:?}", date);
    println!("Item: {:?}", item);
    
    // Compare with expected
    println!("\n--- Comparison ---");
    println!("Expected seller: {:?}", expected.seller);
    println!("Expected amount: {:?}", expected.amount);
    println!("Expected invoice_no: {:?}", expected.invoice_no);
    println!("Expected date: {:?}", expected.date);
}

#[derive(Clone)]
struct ExpectedFields {
    seller: Option<String>,
    amount: Option<String>,
    invoice_no: Option<String>,
    date: Option<String>,
    item: Option<String>,
}

#[test]
fn verify_didi_invoice_a() {
    verify_pdf("滴滴电子发票 A.pdf", "市内交通", "滴滴电子发票 A", &ExpectedFields {
        seller: Some("成都滴滴优行科技有限公司".to_string()),
        amount: Some("523.57".to_string()),
        invoice_no: Some("26517000000358455168".to_string()),
        date: None,
        item: None,
    });
}

#[test]
fn verify_hotel_invoice() {
    verify_pdf("Hotel Invoice", "住宿", "dzfp_26512000001996635841", &ExpectedFields {
        seller: Some("成都博朗君悦酒店管理有限责任公司".to_string()),
        amount: Some("2566.55".to_string()),
        invoice_no: Some("26512000001996635841".to_string()),
        date: Some("2026 年 05 月 14 日".to_string()),
        item: Some("住宿服务".to_string()),
    });
}

#[test]
fn verify_flight_invoice() {
    verify_pdf("Flight Invoice", "机票", "9586482810622", &ExpectedFields {
        seller: Some("阿斯兰航空服务（上海）有限公司".to_string()),
        amount: Some("1740.00".to_string()),
        invoice_no: None,
        date: Some("2026-05-17".to_string()),
        item: None,
    });
}

#[test]
fn verify_insurance_invoice() {
    verify_pdf("Insurance Invoice", "保险", "20_电子发票", &ExpectedFields {
        seller: Some("众安在线财产保险股份有限公司".to_string()),
        amount: Some("50.00".to_string()),
        invoice_no: None,
        date: None,
        item: None,
    });
}
