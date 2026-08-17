#![cfg(feature = "pdfplumber")]

use pdfplumber::{Pdf, TextOptions};
use std::path::Path;

/// Normalize text: keep only CJK characters (U+4E00..U+9FFF) and ASCII alphanumeric.
/// Strips whitespace and all other characters for a clean comparison.
fn normalize_text(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '\u{4e00}'..='\u{9fff}')
                || matches!(c, '\u{3000}'..='\u{303f}') // CJK symbols & punctuation
                || matches!(c, '\u{ff00}'..='\u{ffef}') // Fullwidth forms
                || c == '\u{200b}' // zero-width space
        })
        .collect()
}

/// Extract text via pdfplumber, normalize, and return.
/// Uses catch_unwind to handle panics on problematic PDFs (e.g. CID font parsing).
fn extract_pdfplumber(path: &Path) -> Result<String, String> {
    let pdf = Pdf::open_file(path, None).map_err(|e| format!("pdfplumber open error: {}", e))?;
    let mut all_text = String::new();

    for page_result in pdf.pages_iter() {
        let page = match page_result {
            Ok(p) => p,
            Err(e) => return Err(format!("pdfplumber page error: {}", e)),
        };

        let text = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            page.extract_text(&TextOptions::default())
        }));

        match text {
            Ok(t) => all_text.push_str(&t),
            Err(_) => {
                return Err(
                    "pdfplumber panic during extract_text (likely CID font issue)".to_string(),
                );
            }
        }
    }

    Ok(normalize_text(&all_text))
}

/// Extract text via pdfplumber only. Returns (text, 0.0) — 0.0 is a dummy placeholder
/// since there is no longer a reference extractor to compare against.
fn compare_extraction(pdf_path: &Path) -> (String, f64) {
    let text = extract_pdfplumber(pdf_path).unwrap_or_else(|e| {
        eprintln!("  [WARN] pdfplumber failed: {}", e);
        String::new()
    });
    (text, 0.0)
}

/// Shared test runner: find a PDF, run pdfplumber extraction, assert non-empty result.
fn run_fidelity_test(
    pdf_name: &str,
    pdf_search: Option<&str>,
    base_dir: &Path,
    results: &mut Vec<(String, usize, f64)>,
) {
    // Determine the actual PDF path
    let pdf_path = if let Some(search) = pdf_search {
        let dir = match std::fs::read_dir(base_dir) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  [SKIP] Cannot read directory {:?}: {}", base_dir, e);
                results.push((pdf_name.to_string(), 0, 1.0));
                return;
            }
        };

        let found: Vec<_> = dir
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains(search) {
                    Some(entry.path())
                } else {
                    None
                }
            })
            .collect();

        if found.is_empty() {
            eprintln!(
                "  [SKIP] No file containing '{}' found in {:?}",
                search, base_dir
            );
            results.push((format!("{}* (NOT FOUND)", search), 0, 1.0));
            return;
        }
        found[0].clone()
    } else {
        let path = base_dir.join(pdf_name);
        if !path.exists() {
            eprintln!("  [SKIP] {} not found at {:?}", pdf_name, path);
            results.push((pdf_name.to_string(), 0, 1.0));
            return;
        }
        path
    };

    let display_name = pdf_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| pdf_name.to_string());

    eprintln!("  Testing: {}", display_name);
    let (text, _rate) = compare_extraction(&pdf_path);

    results.push((display_name.clone(), text.chars().count(), 0.0));

    eprintln!("    pdfplumber chars: {}", text.chars().count());

    assert!(
        !text.is_empty(),
        "pdfplumber returned empty text for {}",
        display_name
    );
}

// ─── Individual test functions ──────────────────────────────────────────────

#[test]
fn test_cjk_fidelity_didi_invoice() {
    let mut results = Vec::new();
    run_fidelity_test(
        "滴滴电子发票A.pdf",
        None,
        Path::new("../data/市内交通"),
        &mut results,
    );
}

#[test]
fn test_cjk_fidelity_vat_invoice() {
    let mut results = Vec::new();
    // dzfp_ hotel invoices are in 住宿/; the test needs a dzfp_ that's there
    run_fidelity_test(
        "dzfp_ (glob)",
        Some("dzfp_"),
        Path::new("../data/住宿"),
        &mut results,
    );
}

#[test]
fn test_cjk_fidelity_itinerary() {
    let mut results = Vec::new();
    run_fidelity_test(
        "天府通电子行程单.pdf",
        None,
        Path::new("../data/行程单/天府通"),
        &mut results,
    );
}

#[test]
fn test_cjk_fidelity_flight_ticket() {
    let mut results = Vec::new();
    run_fidelity_test(
        "飞猪 (glob)",
        Some("飞猪"),
        Path::new("../data/机票"),
        &mut results,
    );
}

// ─── Summary test ───────────────────────────────────────────────────────────

#[test]
fn test_cjk_fidelity_summary() {
    let mut results: Vec<(String, usize, f64)> = Vec::new();

    let test_cases: Vec<(&str, Option<&str>)> = vec![
        ("滴滴电子发票A.pdf", None),
        ("dzfp_ (glob)", Some("dzfp_")),
        ("天府通电子行程单.pdf", None),
        ("飞猪 (glob)", Some("飞猪")),
    ];

    let dirs = [
        Path::new("../data/市内交通"),
        Path::new("../data/住宿"),
        Path::new("../data/行程单/天府通"),
        Path::new("../data/机票"),
    ];
    for ((name, search), dir) in test_cases.iter().zip(dirs.iter()) {
        run_fidelity_test(name, *search, dir, &mut results);
    }

    // Print summary table
    println!();
    println!("=== pdfplumber CJK Extraction Summary ===");
    println!("{:<32} {:>11}", "PDF", "pdfplumber");
    println!("{:-<32}-{:-<11}", "", "");

    let mut all_ok = true;
    for (name, pp_count, _rate) in &results {
        if *pp_count == 0 {
            println!("{:<32} {:>11}", name, "SKIP");
            all_ok = false;
            continue;
        }
        println!("{:<32} {:>11}", name, pp_count);
    }
    println!();

    if all_ok {
        println!("All pdfplumber CJK extraction tests PASSED");
    } else {
        println!("Some pdfplumber CJK extraction tests FAILED (empty text)");
    }

    assert!(
        all_ok,
        "One or more pdfplumber CJK extraction tests returned empty text"
    );
}
