#![cfg(feature = "pdfplumber")]
//! 临时诊断：find_tables 在滴滴行程单上的输出形状
//! Run: cargo test --features pdfplumber --test diag_didi_tables -- --nocapture

use pdfplumber::{Pdf, TableSettings};

fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{}/../data/{}", env!("CARGO_MANIFEST_DIR"), normalized).replace('/', "\\")
}

fn diag(pdf_path: &str, label: &str) {
    if !std::path::Path::new(pdf_path).exists() {
        eprintln!("SKIP: {pdf_path} not found");
        return;
    }
    let pdf = match Pdf::open_file(pdf_path, None) {
        Ok(p) => p,
        Err(e) => { eprintln!("open {label} failed: {e}"); return; }
    };
    eprintln!("\n========== {label}: {pdf_path} ==========");
    for (pn, page_result) in pdf.pages_iter().enumerate() {
        let page = match page_result { Ok(p) => p, Err(_) => continue };
        eprintln!("\n--- PAGE {pn} ({}x{}) ---", page.width(), page.height());
        let tables = page.find_tables(&TableSettings::default());
        eprintln!("find_tables: {} tables", tables.len());
        for (ti, t) in tables.iter().enumerate() {
            eprintln!("  TABLE {ti}: bbox=({:.1},{:.1},{:.1},{:.1}) rows={}",
                t.bbox.x0, t.bbox.top, t.bbox.x1, t.bbox.bottom, t.rows.len());
            for (ri, row) in t.rows.iter().take(6).enumerate() {
                let texts: Vec<String> = row.iter()
                    .map(|c| c.text.clone().unwrap_or_default().replace('\n', " "))
                    .collect();
                eprintln!("    row{ri} ({} cells): {:?}", row.len(), texts);
            }
            if t.rows.len() > 6 {
                eprintln!("    ... ({} more rows)", t.rows.len() - 6);
            }
        }
    }
}

#[test]
fn diag_didi_a() {
    diag(&data_path("行程单\\滴滴\\滴滴出行行程报销单A.pdf"), "Didi A");
}

#[test]
fn diag_didi_b() {
    diag(&data_path("行程单\\滴滴\\滴滴出行行程报销单B.pdf"), "Didi B");
}

#[test]
fn diag_tianfu() {
    diag(&data_path("行程单\\天府通\\天府通电子行程单.pdf"), "Tianfu");
}

#[test]
fn diag_gaode() {
    // 高德行程单文件名含金额和行程数
    let dir_path = data_path("行程单\\高德");
    let dir = std::path::Path::new(&dir_path);
    if !dir.exists() {
        eprintln!("SKIP: 高德 dir not found");
        return;
    }
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("pdf") {
            let s = p.to_string_lossy().to_string();
            diag(&s, "Gaode");
        }
    }
}

#[test]
fn diag_tianfu_raw() {
    // 原始版本
    diag(&data_path("行程单\\天府通\\天府通电子行程单_原始.pdf"), "Tianfu Raw");
}
