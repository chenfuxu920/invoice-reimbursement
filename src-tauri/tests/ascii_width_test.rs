#![cfg(feature = "pdfplumber")]
//! ASCII 字符宽度回归测试
//!
//! 验证 `data/火车票/25429165818005131893.pdf` 中英文/数字字符宽度不再过宽。
//!
//! 背景：GBK-EUC-H/Identity-H CID 字体的 /W 数组只覆盖 CJK CID，
//! pdfplumber 用 encoding_rs 解码后按 Unicode 码点查 /W（永远 miss）→
//! fallback /DW=1000（全角）→ ASCII 字符 2× 过宽。
//!
//! 修复：CidFontMetrics::get_width 对 ASCII 范围（0x20-0x7E）fallback 用 0.5× /DW。
//!
//! Run: cargo test --features pdfplumber --test ascii_width_test -- --nocapture

use invoice_reimbursement_lib::pdf::text_extractor::extract_words_raw;
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{DATA_DIR}\\{normalized}")
}

#[test]
fn ascii_chars_half_width() {
    let pdf_path = data_path("火车票/25429165818005131893.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    let words = match extract_words_raw(&pdf_path) {
        Ok(w) => w,
        Err(e) => panic!("extract_words_raw failed: {e}"),
    };

    println!("\n========== ASCII 字符宽度诊断 ==========");
    println!("共 {} 个 raw words", words.len());

    // 收集所有纯 ASCII 字母/数字的 word，记录其宽度
    // 预期：per_char ≈ size * 0.5（中文字体的 ASCII 半宽比例）
    // 修复前：per_char ≈ size（2× 过宽）
    let mut failures = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let text = &w.text;
        if text.is_empty() {
            continue;
        }
        // 只检查纯 ASCII 字母/数字的 word
        let is_pure_ascii = text.chars().all(|c| c.is_ascii_alphanumeric());
        if !is_pure_ascii {
            continue;
        }
        let nchars = text.chars().count();
        if nchars < 3 {
            // 短 word 噪声大，跳过
            continue;
        }
        let width = w.bbox.x1 - w.bbox.x0;
        let per_char = width / nchars as f64;
        println!("  [{i}] {text:?}: w={width:.2} nchars={nchars} per_char={per_char:.2}");

        // 通过 bbox 高度估算字号（CID 字体 ascent+descent ≈ 1.0 em）
        let height = w.bbox.bottom - w.bbox.top;
        let est_size = height; // 近似字号
        let expected_half = est_size * 0.5;
        let expected_full = est_size;

        // 容差 25%（字号估算有误差）
        let half_low = expected_half * 0.75;
        let half_high = expected_half * 1.25;

        if per_char > half_high {
            // 偏向全角 → 修复未生效
            failures.push(format!(
                "[{i}] {text:?}: per_char={per_char:.2} > {half_high:.2} (expected ~{expected_half:.2} = 0.5×size={est_size:.2})"
            ));
        }
        if per_char < half_low {
            failures.push(format!(
                "[{i}] {text:?}: per_char={per_char:.2} < {half_low:.2} (too narrow, expected ~{expected_half:.2})"
            ));
        }
    }

    if !failures.is_empty() {
        eprintln!("\n宽度断言失败 ({} 项):", failures.len());
        for f in &failures {
            eprintln!("  {f}");
        }
        panic!("ASCII 字符宽度断言失败 — 修复未生效或过宽仍存在");
    }

    println!("\n========== 所有 ASCII word 宽度均在半角范围内 ==========\n");
}
