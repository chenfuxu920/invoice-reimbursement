//! Regression: /Contents as indirect-reference-to-array must extract text.
//!
//! gp-template 全电发票 writes `/Contents 28 0 R` where obj 28 is `[29 0 R]`
//! (an array wrapped in an indirect reference). pdfplumber-rs previously only
//! handled `Reference → Stream` and `Array`, failing with
//! "/Contents is not a stream" on the `Reference → Array` shape.
//!
//! Run: cargo test --features pdfplumber --test pdf_contents_array_regression -- --nocapture

#![cfg(feature = "pdfplumber")]

use invoice_reimbursement_lib::pdf::text_extractor::{
    extract_raw_words_debug, extract_text_with_coords_flat, extract_pdf_column_aware,
};
use std::path::Path;

const PDF1: &str = r"C:\Projects\rust-projects\invoice-reimbursement\data\未分类\dzfp_26512000001847622916_中国人民解放军国防科技大学系统工程学院_20260506074422.pdf";
const PDF2: &str = r"C:\Projects\rust-projects\invoice-reimbursement\data\未分类\全电发票(全面数字化电子发票).pdf";

fn check_extracts(label: &str, path: &str) {
    if !Path::new(path).exists() {
        eprintln!("SKIP {label}: PDF not found");
        return;
    }

    // 1. raw words
    let raw = extract_raw_words_debug(path)
        .unwrap_or_else(|e| panic!("{label} extract_raw_words_debug failed: {e}"));
    assert!(
        !raw.is_empty(),
        "{label}: pdfplumber extracted 0 words — /Contents array fix regressed"
    );
    let total_chars: usize = raw.iter().map(|(t, ..)| t.chars().count()).sum();
    assert!(
        total_chars > 50,
        "{label}: expected substantial text, got {total_chars} chars"
    );

    // 2. flat
    let flat = extract_text_with_coords_flat(path)
        .unwrap_or_else(|e| panic!("{label} extract_text_with_coords_flat failed: {e}"));
    assert!(!flat.is_empty(), "{label}: flat extraction empty");

    // 3. column-aware (exercises find_tables + words)
    let ext = extract_pdf_column_aware(path)
        .unwrap_or_else(|e| panic!("{label} extract_pdf_column_aware failed: {e}"));
    assert!(
        !ext.raw_words.is_empty(),
        "{label}: column-aware raw_words empty"
    );

    // 4. content sanity: known invoice text must appear
    let all_text: String = raw.iter().map(|(t, ..)| t.as_str()).collect::<Vec<_>>().join("");
    assert!(
        all_text.contains("发票") || all_text.contains("电子"),
        "{label}: expected invoice keywords, got: {all_text}"
    );

    println!(
        "PASS {label}: {} words, {} chars, {} column-aware items",
        raw.len(),
        total_chars,
        ext.pages.iter().map(|p| p.texts.len()).sum::<usize>()
    );
}

#[test]
fn pdf1_dzfp_contents_indirect_array_extracts() {
    check_extracts("PDF1 dzfp", PDF1);
}

#[test]
fn pdf2_quandian_contents_indirect_array_extracts() {
    check_extracts("PDF2 全电发票", PDF2);
}
