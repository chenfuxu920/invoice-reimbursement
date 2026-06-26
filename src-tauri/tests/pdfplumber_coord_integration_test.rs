#![cfg(feature = "pdfplumber")]

use invoice_reimbursement_lib::models::invoice::InvoiceSource;
use invoice_reimbursement_lib::ocr::OcrTextItem;
use invoice_reimbursement_lib::parser::invoice_parser::parse_invoice_text;
use invoice_reimbursement_lib::parser::itinerary_parser::parse_itinerary_with_coords;
use invoice_reimbursement_lib::pdf::text_extractor::extract_text_with_coords_flat;
use pdfplumber::{Pdf, WordOptions};
use std::path::Path;

/// Helpers directory: src-tauri/tests/../../data/... resolves to project-root/data/
const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

/// Construct box_coords JSON from bounding box coordinates (matches bbox_to_json format).
fn make_box_coords(x0: f64, y0: f64, x1: f64, y1: f64) -> serde_json::Value {
    serde_json::json!({
        "points": [
            {"x": x0, "y": y0},
            {"x": x1, "y": y0},
            {"x": x1, "y": y1},
            {"x": x0, "y": y1}
        ],
        "box_score": 1.0
    })
}

/// Extract words from a PDF using pdfplumber and convert each word to OcrTextItem
/// with bounding box coordinates.
fn pdfplumber_words_to_ocr_items(pdf_path: &str) -> Result<Vec<OcrTextItem>, String> {
    let pdf = Pdf::open_file(pdf_path, None).map_err(|e| format!("Failed to open PDF: {e}"))?;

    let mut items = Vec::new();
    for page_result in pdf.pages_iter() {
        let page = page_result.map_err(|e| format!("Failed to get page: {e}"))?;
        let words = page.extract_words(&WordOptions::default());
        for word in words {
            items.push(OcrTextItem {
                text: word.text.clone(),
                confidence: 1.0,
                box_coords: Some(make_box_coords(
                    word.bbox.x0,
                    word.bbox.top,
                    word.bbox.x1,
                    word.bbox.bottom,
                )),
            });
        }
    }

    Ok(items)
}

/// Resolve a path relative to the project data directory.
fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{DATA_DIR}\\{normalized}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_pdfplumber_coords_produce_valid_items() {
    let pdf_path = data_path("发票与行程单\\滴滴电子发票A.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    let items = pdfplumber_words_to_ocr_items(&pdf_path)
        .expect("pdfplumber should extract words from the invoice PDF");

    // At least 10 words extracted from a real invoice
    assert!(
        items.len() >= 10,
        "Expected >= 10 items from invoice PDF, got {}",
        items.len()
    );

    // Every item must have box_coords
    for (i, item) in items.iter().enumerate() {
        assert!(
            item.box_coords.is_some(),
            "Item[{i}] '{}' missing box_coords",
            item.text
        );
    }

    // Every box_coords must have a points array with 4 elements
    for (i, item) in items.iter().enumerate() {
        let coords = item.box_coords.as_ref().expect("checked above");
        let points = coords
            .get("points")
            .and_then(|v| v.as_array())
            .expect("box_coords must have 'points' array");
        assert_eq!(
            points.len(),
            4,
            "Item[{i}] '{}' box_coords has {} points (expected 4)",
            item.text,
            points.len()
        );
    }

    // Coordinates should be in valid PDF unit range (A4 ~595 units wide, < 1000)
    for (i, item) in items.iter().enumerate() {
        let coords = item.box_coords.as_ref().expect("checked above");
        let points = coords.get("points").and_then(|v| v.as_array()).unwrap();
        for (p_idx, point) in points.iter().enumerate() {
            let x = point
                .get("x")
                .and_then(|v| v.as_f64())
                .expect("point must have x");
            let y = point
                .get("y")
                .and_then(|v| v.as_f64())
                .expect("point must have y");
            assert!(
                x >= 0.0 && x <= 1000.0,
                "Item[{i}] point[{p_idx}] x={x} out of PDF range [0, 1000]"
            );
            assert!(
                y >= 0.0 && y <= 1000.0,
                "Item[{i}] point[{p_idx}] y={y} out of PDF range [0, 1000]"
            );
        }
    }

    // Print first 5 items as diagnostic
    println!("=== First 5 pdfplumber words (with coords) ===");
    for (i, item) in items.iter().take(5).enumerate() {
        let coords = item.box_coords.as_ref().unwrap();
        let points = coords.get("points").and_then(|v| v.as_array()).unwrap();
        let (x0, y0, x1, y1) = (
            points[0]["x"].as_f64().unwrap(),
            points[0]["y"].as_f64().unwrap(),
            points[2]["x"].as_f64().unwrap(),
            points[2]["y"].as_f64().unwrap(),
        );
        println!(
            "  [{i}] text='{}' conf={} bbox=({x0:.1},{y0:.1})-({x1:.1},{y1:.1})",
            item.text, item.confidence
        );
    }
}

#[test]
fn test_pdfplumber_coords_flow_through_invoice_parser() {
    let pdf_path = data_path("发票与行程单\\滴滴电子发票A.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    let items = extract_text_with_coords_flat(&pdf_path)
        .expect("extract_text_with_coords_flat should extract line-level items from invoice PDF");

    // Use the full parse path — this internally calls extract_seller_by_coords
    let source = InvoiceSource::Pdf(pdf_path.clone());
    let result = parse_invoice_text(&items, source);

    match result {
        Ok(invoice) => {
            println!("=== Invoice parser result (line-level items) ===");
            println!("  seller_name:   '{}'", invoice.seller_name);
            println!("  invoice_no:    '{}'", invoice.invoice_number);
            println!("  total_amount:  {:.2}", invoice.amount);
            println!("  date:          {}", invoice.date);
            println!("  item_name:     '{}'", invoice.item_name);

            // With line-level items (merge_words_into_lines), the parser should
            // successfully extract invoice_number and ideally seller_name.
            assert!(
                !invoice.invoice_number.is_empty(),
                "invoice_number should be non-empty with line-level items from merge_words_into_lines"
            );
            if invoice.seller_name.is_empty() {
                eprintln!(
                    "NOTE: seller_name is empty even with line-level items — may need further investigation"
                );
            }
        }
        Err(e) => {
            panic!(
                "parse_invoice_text should succeed with line-level items from extract_text_with_coords_flat, but got error: {e}"
            );
        }
    }
}

#[test]
fn test_pdfplumber_coords_flow_through_itinerary_parser() {
    let pdf_path = data_path("发票与行程单\\天府通电子行程单.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    let items = extract_text_with_coords_flat(&pdf_path)
        .expect("extract_text_with_coords_flat should extract line-level items from itinerary PDF");

    // parse_itinerary_with_coords is public — call it directly
    // It will fall back to parse_itinerary_text if coordinate data is insufficient.
    let itineraries = parse_itinerary_with_coords(&items);

    println!("=== Itinerary parser result ===");
    println!("  Number of itineraries: {}", itineraries.len());

    if itineraries.is_empty() {
        eprintln!("NOTE: No itineraries parsed — coords-based parsing may need tuning for this PDF");
    } else {
        let first = &itineraries[0];
        println!(
            "  First itinerary: date_time='{}' provider='{}' amount={:.2}",
            first.date_time, first.provider, first.amount
        );
    }

    // Print all itineraries for diagnostics
    for (i, it) in itineraries.iter().enumerate() {
        println!(
            "  [{i}] date_time='{}' provider='{}' pickup='{}' dropoff='{}' amount={:.2}",
            it.date_time, it.provider, it.pickup, it.dropoff, it.amount
        );
    }
}

#[test]
fn test_coord_scale_is_pdf_units_not_pixels() {
    let pdf_path = data_path("发票与行程单\\滴滴电子发票A.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    let pdf = Pdf::open_file(&pdf_path, None).expect("should open PDF");

    let mut max_x = f64::NEG_INFINITY;
    let mut min_x = f64::INFINITY;
    let mut page_widths = Vec::new();

    for page_result in pdf.pages_iter() {
        let page = page_result.expect("should get page");
        // The page width can be inferred from extract_words bounding boxes
        let words = page.extract_words(&WordOptions::default());
        if words.is_empty() {
            continue;
        }
        let pw = words
            .iter()
            .map(|w| w.bbox.x1)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);
        page_widths.push(pw);

        for word in &words {
            if word.bbox.x1 > max_x {
                max_x = word.bbox.x1;
            }
            if word.bbox.x0 < min_x {
                min_x = word.bbox.x0;
            }
        }
    }

    println!("=== Coordinate scale check ===");
    println!("  Number of pages: {}", page_widths.len());
    for (i, pw) in page_widths.iter().enumerate() {
        println!("  Page[{i}] max word x: {pw:.2}");
    }
    println!("  Overall max X: {max_x:.2}");
    println!("  Overall min X: {min_x:.2}");

    // A4 is ~595 units wide. Even for larger PDFs (e.g. A3: ~842),
    // we should never see pixel-space coordinates (> 2000 would be pixel scale).
    assert!(
        max_x < 2000.0,
        "max_x={max_x:.2} >= 2000 — this looks like pixel-space coordinates, not PDF units"
    );
    assert!(
        max_x < 1000.0,
        "max_x={max_x:.2} >= 1000 — unusual; expected PDF unit coordinates (< 1000 for A4/A3)"
    );
}
