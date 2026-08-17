//! zpdf 渲染 CJK PDF 时 ASCII 字符宽度通用回归测试
//!
//! 验证 zpdf-fork 的 `CidWidths::get` 修复（CID 范围 `1..=0x7E`）对所有
//! CJK PDF 端到端生效，不局限于单个火车票 PDF。
//!
//! 修复作用域（字体库级别，非 PDF 级别）：
//!   • Predefined legacy CJK CMaps (GBK-EUC-H/B5pc/90ms-RKSJ/KSC-EUC/…):
//!     CidCMap::legacy() 对所有 6 种 LegacyEncoding 都装 cid_ranges
//!     [(1, 0x20, 0x7E, 1)]，ASCII 字节 0x20-0x7E → CID 1-95
//!   • Identity-H: CID = GID，ASCII 0x20-0x7E → CID 0x20-0x7E
//!   • 修复范围 (1..=0x7E) 覆盖两者；CJK CID 0 或 GID >0x7E 走 /DW（全角，正确）
//!
//! 测试策略：遍历 data/ 下所有 .pdf，对每个 PDF 提取 TextSpan，
//! 统计纯 ASCII span 的 per_char/size 比值。ratio ≈ 0.5 = 半角（正确），
//! ratio ≈ 1.0 = 全角（2× 过宽，修复未生效）。
//!
//! Run: cargo test --test zpdf_ascii_width_test -- --nocapture

use std::path::{Path, PathBuf};
use zpdf::{ContentInterpreter, PdfDocument};

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

fn collect_pdfs(root: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_pdfs(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("pdf") {
                out.push(path);
            }
        }
    }
}

/// 提取一个 PDF 所有页面的 TextSpan。
fn extract_spans(pdf_path: &Path) -> Result<Vec<zpdf::TextSpan>, String> {
    let data = std::fs::read(pdf_path).map_err(|e| format!("read: {e}"))?;
    let doc = PdfDocument::open(data).map_err(|e| format!("open: {e:?}"))?;

    let mut all = Vec::new();
    for i in 0..doc.page_count() {
        let page = doc.page(i).map_err(|e| format!("page {i}: {e:?}"))?;
        let mut fonts = doc.load_page_fonts(&page);
        let content = doc
            .page_content_bytes(&page)
            .map_err(|e| format!("content: {e:?}"))?;
        let annotations = doc.page_annotations(&page);
        let mut img_cache = zpdf::ImageCache::new();
        let mut spans: Vec<zpdf::TextSpan> = Vec::new();
        let _ = ContentInterpreter::new(page.effective_box())
            .with_page_rotation(page.rotate)
            .with_fonts(&mut fonts)
            .with_document(doc.file(), &page.resources)
            .with_images(&mut img_cache)
            .with_annotations(&annotations)
            .with_text_sink(&mut spans)
            .interpret(&content);
        all.extend(spans);
    }
    Ok(all)
}

/// 对纯 ASCII 字母数字 span（≥3 字符）统计 ratio = per_char / size。
/// 返回 (checked_count, failure_list)。ratio > 0.75 视为全角（修复未生效）。
fn audit_spans(spans: &[zpdf::TextSpan]) -> (usize, Vec<String>) {
    let mut checked = 0usize;
    let mut failures = Vec::new();
    for s in spans {
        let text = &s.text;
        if text.is_empty() || s.size <= 0.0 {
            continue;
        }
        if !text.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        let nchars = text.chars().count();
        if nchars < 3 {
            continue;
        }
        let advance = s.advance.abs();
        let per_char = advance / nchars as f64;
        let ratio = per_char / s.size as f64;
        checked += 1;
        if ratio > 0.75 {
            failures.push(format!(
                "{text:?}: ratio={ratio:.3} per_char={per_char:.2} size={:.2}",
                s.size
            ));
        }
    }
    (checked, failures)
}

#[test]
fn zpdf_ascii_half_width_across_all_pdfs() {
    let root = Path::new(DATA_DIR);
    if !root.is_dir() {
        eprintln!("SKIP: data dir not found at {}", root.display());
        return;
    }
    let mut pdfs = Vec::new();
    collect_pdfs(root, &mut pdfs);
    if pdfs.is_empty() {
        eprintln!("SKIP: no PDFs under {}", root.display());
        return;
    }

    println!(
        "\n========== zpdf ASCII 宽度全量审计（{} 个 PDF）==========",
        pdfs.len()
    );

    let mut total_checked = 0usize;
    let mut total_failures = 0usize;
    let mut pdfs_with_failures = 0usize;
    let mut pdfs_skipped = 0usize;

    for pdf in &pdfs {
        let rel = pdf.strip_prefix(root).unwrap_or(pdf).display().to_string();
        let spans = match extract_spans(pdf) {
            Ok(s) => s,
            Err(e) => {
                println!("  [SKIP] {rel}: {e}");
                pdfs_skipped += 1;
                continue;
            }
        };
        let (checked, failures) = audit_spans(&spans);
        total_checked += checked;
        total_failures += failures.len();
        if !failures.is_empty() {
            pdfs_with_failures += 1;
        }
        let status = if checked == 0 {
            "—".to_string()
        } else if failures.is_empty() {
            "OK".to_string()
        } else {
            format!("FAIL({})", failures.len())
        };
        println!(
            "  [{status:>10}] {rel}: spans={} checked={checked} fails={}",
            spans.len(),
            failures.len()
        );
        if !failures.is_empty() && failures.len() <= 5 {
            for f in &failures {
                println!("      - {f}");
            }
        }
    }

    println!("\n========== 汇总 ==========");
    println!("  PDF 总数:           {}", pdfs.len());
    println!("  跳过 (解析失败):    {pdfs_skipped}");
    println!("  有效审计 PDF:       {}", pdfs.len() - pdfs_skipped);
    println!("  检查 ASCII span:    {total_checked}");
    println!("  失败 span:          {total_failures}");
    println!("  失败 PDF 数:        {pdfs_with_failures}");

    if total_checked == 0 {
        eprintln!("WARN: 未找到任何符合条件的 ASCII span，测试无覆盖");
        return;
    }

    if total_failures > 0 {
        panic!(
            "ASCII 字符宽度断言失败 — {total_failures} 个 span 仍为全角宽度（涉及 {pdfs_with_failures} 个 PDF），修复未通用生效"
        );
    }

    println!("\n========== 所有 PDF 的 ASCII span 宽度均为半角范围内 ==========\n");
}
