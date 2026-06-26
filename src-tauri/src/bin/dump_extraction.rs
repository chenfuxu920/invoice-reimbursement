//! 诊断工具：对目录下每个 PDF 同时输出 pdfplumber(带坐标) 与 parangi(纯文本) 提取结果，
//! 并分别用 parse_invoice_text 解析，便于交叉核对当前发票提取错误。
//!
//! 用法: dump_extraction <pdf_dir>
//! 构建: cargo run --release --bin dump_extraction -- <pdf_dir>   (默认启用 pdfplumber feature)

use invoice_reimbursement_lib::models::invoice::{Invoice, InvoiceSource};
use invoice_reimbursement_lib::parser::invoice_parser::parse_invoice_text;
use invoice_reimbursement_lib::pdf::text_extractor::{
    classify_pdf_document_type, extract_text_from_pdf, has_sufficient_text, is_garbled_items, PdfDocumentType,
};
use invoice_reimbursement_lib::ocr::OcrTextItem;
use std::path::Path;

#[cfg(feature = "pdfplumber")]
use invoice_reimbursement_lib::pdf::text_extractor::{extract_raw_words_debug, extract_text_with_coords_flat, extract_words_raw};
#[cfg(feature = "pdfplumber")]
use invoice_reimbursement_lib::parser::layout_extractor;

/// 从 box_coords (serde_json::Value) 中提取 (x0, top, x1, bottom) 概要
fn coord_summary(box_coords: &Option<serde_json::Value>) -> String {
    match box_coords {
        Some(v) => {
            let pts = match v.get("points").and_then(|p| p.as_array()) {
                Some(a) => a,
                None => return "(-,-)".to_string(),
            };
            let xs: Vec<f64> = pts.iter().filter_map(|p| p.get("x").and_then(|x| x.as_f64())).collect();
            let ys: Vec<f64> = pts.iter().filter_map(|p| p.get("y").and_then(|y| y.as_f64())).collect();
            if xs.is_empty() || ys.is_empty() {
                return "(-,-)".to_string();
            }
            let x0 = xs.iter().cloned().fold(f64::INFINITY, f64::min);
            let x1 = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let y0 = ys.iter().cloned().fold(f64::INFINITY, f64::min);
            let y1 = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            format!("({:.0},{:.0})-({:.0},{:.0})", x0, y0, x1, y1)
        }
        None => "(无坐标)".to_string(),
    }
}

fn print_items(label: &str, items: &[OcrTextItem]) {
    println!("--- {} [{} 项] ---", label, items.len());
    for (i, item) in items.iter().enumerate() {
        let c = coord_summary(&item.box_coords);
        println!("  [{}] {} {}", i, c, item.text);
    }
}

fn print_invoice(label: &str, result: &Result<Invoice, String>) {
    println!("--- {} ---", label);
    match result {
        Ok(inv) => {
            println!("  seller_name  : [{}]", inv.seller_name);
            println!("  amount       : {}", inv.amount);
            println!("  invoice_number: [{}]", inv.invoice_number);
            println!("  date         : {}", inv.date);
            println!("  item_name    : [{}]", inv.item_name);
            println!("  category     : {:?}", inv.category);
            if let Some(d) = &inv.departure_city {
                println!("  departure    : {}", d);
            }
            if let Some(a) = &inv.arrival_city {
                println!("  arrival      : {}", a);
            }
            if !inv.remarks.is_empty() {
                println!("  remarks      : [{}]", inv.remarks);
            }
        }
        Err(e) => println!("  ERR: {}", e),
    }
}

fn doc_type_str(t: &PdfDocumentType) -> &'static str {
    match t {
        PdfDocumentType::Invoice => "Invoice",
        PdfDocumentType::Itinerary => "Itinerary",
        PdfDocumentType::Bill => "Bill",
        PdfDocumentType::Unknown => "Unknown",
    }
}

fn process_pdf(path: &Path) {
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let path_str = path.to_string_lossy().to_string();
    println!("\n================================================================");
    println!("FILE: {}", name);
    println!("================================================================");

    // parangi 纯文本
    let parangi_items = match extract_text_from_pdf(&path_str) {
        Ok(items) => items,
        Err(e) => {
            println!("[parangi] 失败: {}", e);
            Vec::new()
        }
    };
    print_items("parangi (纯文本)", &parangi_items);
    println!("  parangi 文本充足(>=20): {}", has_sufficient_text(&parangi_items, 20));
    println!("  parangi 乱码检测: {}", is_garbled_items(&parangi_items, 0.3));
    println!("  分类(parangi): {}", doc_type_str(&classify_pdf_document_type(&parangi_items)));
    let src = InvoiceSource::Pdf(path_str.clone());
    print_invoice("解析(parangi)", &parse_invoice_text(&parangi_items, src.clone()));

    #[cfg(feature = "pdfplumber")]
    {
        // 先打印原始 Word（未经 merge），验证分栏检测可行性
        match extract_raw_words_debug(&path_str) {
            Ok(raw_words) => {
                println!("\n--- pdfplumber 原始 Word [{} 个，未合并] ---", raw_words.len());
                for (i, (text, x0, top, x1, bottom, pn)) in raw_words.iter().enumerate() {
                    let w = x1 - x0;
                    println!("  [{}] p{} ({:.0},{:.0})-({:.0},{:.0}) w={:.0} {}",
                        i, pn, x0, top, x1, bottom, w, text);
                }
            }
            Err(e) => println!("\n--- pdfplumber 原始 Word 失败: {} ---", e),
        }

        let pdfplumber_items = match extract_text_with_coords_flat(&path_str) {
            Ok(items) => items,
            Err(e) => {
                println!("\n[pdfplumber] 失败: {}", e);
                Vec::new()
            }
        };
        print_items("pdfplumber (带坐标)", &pdfplumber_items);
        println!("  pdfplumber 文本充足(>=20): {}", has_sufficient_text(&pdfplumber_items, 20));
        println!("  pdfplumber 乱码检测: {}", is_garbled_items(&pdfplumber_items, 0.3));
        println!("  分类(pdfplumber): {}", doc_type_str(&classify_pdf_document_type(&pdfplumber_items)));
        print_invoice("解析(pdfplumber)", &parse_invoice_text(&pdfplumber_items, src));

        // 坐标 seller 和 amount 提取（原始 Word，未经 merge）
        match extract_words_raw(&path_str) {
            Ok(words) => {
                let seller = layout_extractor::extract_seller_by_raw_coords(&words);
                println!("--- 坐标seller(原始Word) ---");
                println!("  seller_name  : [{}]", seller);

                let amount = layout_extractor::extract_amount_by_coords(&words);
                println!("--- 坐标amount(原始Word) ---");
                match amount {
                    Some(amt) => println!("  amount       : {}", amt),
                    None => println!("  amount       : None (未找到价税合计锚点)"),
                }
            }
            Err(e) => println!("--- 坐标提取 失败: {} ---", e),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: dump_extraction <pdf_dir>");
        std::process::exit(1);
    }
    let dir = &args[1];
    let dir_path = Path::new(dir);
    if !dir_path.is_dir() {
        eprintln!("不是目录: {}", dir);
        std::process::exit(1);
    }

    let mut pdfs: Vec<std::path::PathBuf> = std::fs::read_dir(dir_path)
        .expect("read_dir 失败")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "pdf"))
        .collect();
    pdfs.sort();

    println!("共 {} 个 PDF，目录: {}", pdfs.len(), dir);

    for p in &pdfs {
        process_pdf(p);
    }
}
