#![cfg(feature = "pdfplumber")]
//! 验证单元格引导提取：从发票 PDF 的 find_tables 结果中提取字段。
//! Run: cargo test --features pdfplumber --test cell_extract_verify_test -- --nocapture

use invoice_reimbursement_lib::pdf::text_extractor::{extract_pdf_column_aware, PdfExtraction};
use invoice_reimbursement_lib::parser::cell_extractor::{extract_fields_from_tables, CellInvoiceFields};
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

fn verify_pdf(name: &str, pdf_path: &str) {
    if !Path::new(pdf_path).exists() {
        println!("SKIP: {name} — file not found");
        return;
    }
    let extraction = match extract_pdf_column_aware(pdf_path) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("  [{name}] EXTRACT FAIL: {e}");
            return;
        }
    };
    if !has_invoice_cells(&extraction) {
        return; // itinerary/bill — not an invoice
    }
    let fields: CellInvoiceFields = extract_fields_from_tables(&extraction.tables);

    let seller = fields.seller_name.as_deref().unwrap_or("");
    assert!(!seller.is_empty(), "[{name}] seller_name is None");
    assert!(seller.chars().count() >= 3, "[{name}] seller_name too short: '{seller}'");
    assert!(
        !seller.chars().all(|c| c.is_ascii_alphanumeric()),
        "[{name}] seller_name looks like tax ID: '{seller}'"
    );
    let amount = fields.amount.unwrap_or(0.0);
    assert!(amount > 0.0, "[{name}] amount is None or zero");
    assert!(fields.item_name.is_some(), "[{name}] item_name is None");

    println!("  [{name}] ✓ seller={seller}, amount={amount}, item={}", fields.item_name.as_deref().unwrap_or(""));
}

fn has_invoice_cells(extraction: &PdfExtraction) -> bool {
    for tables in &extraction.tables {
        for table in tables {
            for row in &table.rows {
                for cell in row {
                    let t: String = cell.text.chars().filter(|c| !c.is_whitespace()).collect();
                    if t.contains("销售方") || t.contains("购买方") || t.contains("价税合计") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

const INVOICE_TYPE_DIRS: &[&str] = &[
    "市内交通",
    "机票",
    "退改签",
    "住宿",
    "保险",
    "通行费",
    "其他发票",
    "未分类",
];

fn scan_and_verify_dir(dir: &str, label: &str) {
    let full_dir = format!("{DATA_DIR}\\{dir}");
    let entries = match std::fs::read_dir(&full_dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "pdf") {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            if filename.contains("行程报销单") || filename.contains("行程单") || filename.contains("结账单") {
                continue;
            }
            verify_pdf(&format!("{label}/{filename}"), &path.to_string_lossy());
        }
    }
}

#[test]
fn verify_all_invoices() {
    for type_dir in INVOICE_TYPE_DIRS {
        scan_and_verify_dir(type_dir, type_dir);
    }
}
