#![cfg(feature = "pdfplumber")]
//! 验证单元格引导提取：从发票 PDF 的 find_tables 结果中提取字段。
//! Run: cargo test --features pdfplumber --test cell_extract_verify_test -- --nocapture

use invoice_reimbursement_lib::pdf::text_extractor::{extract_pdf_column_aware, TableInfo};
use invoice_reimbursement_lib::parser::cell_extractor::{extract_fields_from_tables, CellInvoiceFields};
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{DATA_DIR}\\{normalized}")
}

fn verify_pdf(name: &str, relative: &str) {
    println!("\n========== {name} ==========");
    let pdf_path = data_path(relative);
    if !Path::new(&pdf_path).exists() {
        println!("SKIP: file not found");
        return;
    }
    let extraction = match extract_pdf_column_aware(&pdf_path) {
        Ok(e) => e,
        Err(e) => {
            println!("EXTRACT FAIL: {e}");
            return;
        }
    };
    println!("tables: {} pages", extraction.tables.len());
    for (pi, tables) in extraction.tables.iter().enumerate() {
        println!("  page {pi}: {} tables", tables.len());
    }
    let fields: CellInvoiceFields = extract_fields_from_tables(&extraction.tables);
    println!("seller_name: {:?}", fields.seller_name);
    println!("amount: {:?}", fields.amount);
    println!("item_name: {:?}", fields.item_name);
    let remarks_preview: String = fields.remarks.as_deref().map(|s| s.chars().take(80).collect()).unwrap_or_default();
    println!("remarks: {}", remarks_preview);
    println!("hotel_detail: {:?}", fields.hotel_detail);
}

#[test]
fn verify_all_invoices() {
    verify_pdf("Didi A", "发票与行程单/滴滴电子发票A.pdf");
    verify_pdf("Didi B", "发票与行程单/滴滴电子发票B.pdf");
    verify_pdf(
        "VAT 国防科大1",
        "发票与行程单/dzfp_26512000001728418261_中国人民解放军国防科技大学系统工程学院_20260427084626.pdf",
    );
    verify_pdf("043002200111", "发票与行程单/043002200111_32092584.pdf");
    verify_pdf("26517000000356420562", "发票与行程单/26517000000356420562.pdf");
}
