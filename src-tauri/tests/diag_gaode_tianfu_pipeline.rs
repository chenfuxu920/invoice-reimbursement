#![cfg(feature = "pdfplumber")]
//! 诊断：高德/天府通行程单 pipeline 完整解析结果
//! Run: cargo test --features pdfplumber --test diag_gaode_tianfu_pipeline -- --nocapture

use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::pdf::invoice_pipeline::parse_itinerary_from_pdf;
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");
const MODELS_DIR: &str = "models";

fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{DATA_DIR}\\{normalized}")
}

fn try_init_engine() -> Option<OcrEngine> {
    let models_path = Path::new(MODELS_DIR);
    if !models_path.exists() {
        eprintln!("  [SKIP] Models directory '{MODELS_DIR}' not found");
        return None;
    }
    match OcrEngine::new(MODELS_DIR) {
        Ok(engine) => Some(engine),
        Err(e) => {
            eprintln!("  [SKIP] OcrEngine::new('{MODELS_DIR}') failed: {e}");
            None
        }
    }
}

fn diag_pipeline(pdf_path: &str, label: &str, engine: &mut OcrEngine) {
    if !Path::new(pdf_path).exists() {
        eprintln!("\nSKIP: {pdf_path} not found");
        return;
    }
    eprintln!("\n========== {label}: {pdf_path} ==========");
    match parse_itinerary_from_pdf(pdf_path, engine) {
        Ok(doc) => {
            eprintln!("  ✓ {} 条行程, total={:.2}, printed_total={:?}",
                doc.itineraries.len(), doc.total_amount, doc.printed_total);
            for (i, it) in doc.itineraries.iter().enumerate() {
                eprintln!("    [{i}] time='{}' provider='{}' amount={:.2} pickup='{}' dropoff='{}'",
                    it.date_time, it.provider, it.amount, it.pickup, it.dropoff);
            }
        }
        Err(e) => eprintln!("  ✗ FAILED: {e}"),
    }
}

#[test]
fn diag_gaode_pipeline() {
    let mut engine = match try_init_engine() {
        Some(e) => e,
        None => return,
    };
    let dir = data_path("行程单\\高德");
    if !Path::new(&dir).exists() {
        eprintln!("SKIP: 高德 dir not found");
        return;
    }
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("pdf") {
            diag_pipeline(&p.to_string_lossy(), "Gaode", &mut engine);
        }
    }
}

#[test]
fn diag_tianfu_pipeline() {
    let mut engine = match try_init_engine() {
        Some(e) => e,
        None => return,
    };
    diag_pipeline(&data_path("行程单\\天府通\\天府通电子行程单.pdf"), "Tianfu 处理版", &mut engine);
    diag_pipeline(&data_path("行程单\\天府通\\天府通电子行程单_原始.pdf"), "Tianfu 原始版", &mut engine);
}
