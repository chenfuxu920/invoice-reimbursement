#![cfg(feature = "pdfplumber")]

use invoice_reimbursement_lib::pdf::debug_extract::{debug_extract_texts, DebugTextItem};
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{DATA_DIR}\\{normalized}")
}

#[test]
fn test_debug_extract_returns_structure_with_scaled_coords() {
    let pdf_path = data_path("发票与行程单\\滴滴电子发票A.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    // ocr_engine = None：OCR 数组应为空，不阻塞 pdfplumber/zpdf
    let result = debug_extract_texts(&pdf_path, 200, None).expect("extract should succeed");

    assert!(!result.pages.is_empty(), "should have at least one page");
    let page = &result.pages[0];
    assert!(!page.image.is_empty(), "image base64 should be present");
    assert!(
        page.image.starts_with("data:image/png;base64,"),
        "image should be png data uri"
    );
    assert!(
        page.width > 0 && page.height > 0,
        "image dimensions should be set"
    );

    // pdfplumber 应提取到文字（这是文字型 PDF）
    assert!(!page.pdfplumber.is_empty(), "pdfplumber should extract words");

    // 所有 pdfplumber 坐标必须在图片像素范围内（验证缩放正确）
    for item in &page.pdfplumber {
        assert_in_bounds(item, page.width, page.height, "pdfplumber");
    }

    // zpdf 也应提取到文字
    assert!(!page.zpdf.is_empty(), "zpdf should extract words");
    for item in &page.zpdf {
        assert_in_bounds(item, page.width, page.height, "zpdf");
    }

    // OCR 引擎未提供，应为空数组（不报错）
    assert!(page.ocr.is_empty(), "ocr should be empty when engine is None");

    // 日志应存在（至少有渲染日志）
    assert!(!result.logs.pdfplumber.is_empty(), "pdfplumber logs should exist");
    assert!(!result.logs.zpdf.is_empty(), "zpdf logs should exist");
    assert!(!result.logs.ocr.is_empty(), "ocr logs should exist (degradation reason)");

    // 图形元素字段应存在（线条/矩形，发票通常有表格线条）
    let _ = &page.lines;
    let _ = &page.rects;
}

#[test]
fn test_debug_extract_different_dpi_scales_coords_proportionally() {
    let pdf_path = data_path("发票与行程单\\滴滴电子发票A.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    let r150 = debug_extract_texts(&pdf_path, 150, None).unwrap();
    let r300 = debug_extract_texts(&pdf_path, 300, None).unwrap();

    let p150 = &r150.pages[0];
    let p300 = &r300.pages[0];

    // 图片尺寸应随 DPI 线性缩放
    let w_ratio = p300.width as f64 / p150.width as f64;
    assert!(
        (w_ratio - 2.0).abs() < 0.05,
        "300/150 width ratio should be ~2.0, got {w_ratio}"
    );

    // pdfplumber 第一个文字框的 x 也应随 DPI 线性缩放
    let x150 = p150.pdfplumber[0].x;
    let x300 = p300.pdfplumber[0].x;
    let x_ratio = x300 / x150;
    assert!(
        (x_ratio - 2.0).abs() < 0.05,
        "pdfplumber x should scale ~2x, got {x_ratio}"
    );
}

fn assert_in_bounds(item: &DebugTextItem, w: u32, h: u32, label: &str) {
    let (w, h) = (w as f64, h as f64);
    // Generous tolerance: PDF text elements can extend beyond the rendered crop box edge.
    // The test validates scaling sanity, not exact pixel-perfect containment.
    let tol = 60.0;
    assert!(
        item.x >= -tol && item.x <= w + tol,
        "{label} x={} out of [0, {}]",
        item.x,
        w
    );
    assert!(
        item.y >= -tol && item.y <= h + tol,
        "{label} y={} out of [0, {}]",
        item.y,
        h
    );
    assert!(
        item.w > 0.0 && item.h > 0.0,
        "{label} w/h ({},{}) should be positive",
        item.w,
        item.h
    );
    assert!(
        item.x + item.w <= w + tol,
        "{label} x+w={} exceeds {}",
        item.x + item.w,
        w
    );
    assert!(
        item.y + item.h <= h + tol,
        "{label} y+h={} exceeds {}",
        item.y + item.h,
        h
    );
}
