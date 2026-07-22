//! zpdf 渲染 Identity-H 子集字体时 CJK 字形宽度回归测试
//!
//! 验证 glyph_advance() 修复：Identity-H 子集字体中 CJK 字形 GID 是小整数
//! （如 61 字形子集的 GID 1-60），旧代码的 ASCII 半角启发式 (1..=0x7E)
//! 会错误地把这些 CJK 字形宽度减半，导致文字重叠。
//!
//! 修复后 glyph_advance() 对 Identity-H 字体优先查嵌入 TTF 的 hmtx 表，
//! 得到正确的全角宽度（~1000 units = 1.0× font_size），不再走启发式。
//!
//! 测试策略：对 dzfp 全电发票 PDF 提取 TextSpan，统计纯 CJK span 的
//! per_char/size 比值。ratio ≈ 1.0 = 全角（正确），ratio ≈ 0.5 = 半角（bug）。
//!
//! Run: cargo test --test zpdf_cjk_identity_h_width_test -- --nocapture

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

fn extract_spans(pdf_path: &Path) -> Result<Vec<zpdf::TextSpan>, String> {
    let data = std::fs::read(pdf_path).map_err(|e| format!("read: {e}"))?;
    let doc = PdfDocument::open(data).map_err(|e| format!("open: {e:?}"))?;
    let mut all = Vec::new();
    for i in 0..doc.page_count() {
        let page = doc.page(i).map_err(|e| format!("page {i}: {e:?}"))?;
        let mut fonts = doc.load_page_fonts(&page);
        let content = doc.page_content_bytes(&page).map_err(|e| format!("content: {e:?}"))?;
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

/// 判断字符是否为 CJK（中日韩统一表意文字 + 全角标点）
fn is_cjk(c: char) -> bool {
    let cp = c as u32;
    // CJK Unified Ideographs
    (0x4E00..=0x9FFF).contains(&cp)
    // CJK Extension A
    || (0x3400..=0x4DBF).contains(&cp)
    // CJK Compatibility Ideographs
    || (0xF900..=0xFAFF).contains(&cp)
    // Fullwidth ASCII / Fullwidth punct
    || (0xFF00..=0xFFEF).contains(&cp)
    // CJK punct: 、。〈〉《》「」『』【】 etc.
    || (0x3000..=0x303F).contains(&cp)
}

/// 对纯 CJK span（≥2 字符，无 ASCII）统计 ratio = per_char / size。
/// ratio ≈ 1.0 = 全角（正确），ratio < 0.7 = 半角（bug 未修复）。
/// 返回 (checked_count, failure_list)。
fn audit_cjk_spans(spans: &[zpdf::TextSpan]) -> (usize, Vec<String>) {
    let mut checked = 0usize;
    let mut failures = Vec::new();
    for s in spans {
        let text = &s.text;
        if text.is_empty() || s.size <= 0.0 {
            continue;
        }
        // 只检查纯 CJK span（不含 ASCII 字母数字）
        if !text.chars().all(is_cjk) {
            continue;
        }
        let nchars = text.chars().count();
        if nchars < 2 {
            continue;
        }
        let advance = s.advance.abs();
        let per_char = advance / nchars as f64;
        let ratio = per_char / s.size as f64;
        checked += 1;
        // CJK 全角宽度 ratio 应 ≈ 1.0。允许 0.7-1.3 范围（字体差异）。
        // ratio < 0.7 说明被错误半角化（bug 未修复）
        if ratio < 0.7 {
            failures.push(format!(
                "{text:?}: ratio={ratio:.3} per_char={per_char:.2} size={:.2} (应为~1.0全角)",
                s.size
            ));
        }
    }
    (checked, failures)
}

#[test]
fn zpdf_cjk_identity_h_full_width_across_all_pdfs() {
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

    println!("\n========== zpdf CJK 全角宽度审计（{} 个 PDF）==========", pdfs.len());

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
        let (checked, failures) = audit_cjk_spans(&spans);
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
        println!("  [{status:>10}] {rel}: spans={} cjk_checked={checked} fails={}", spans.len(), failures.len());
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
    println!("  检查 CJK span:      {total_checked}");
    println!("  失败 span:          {total_failures}");
    println!("  失败 PDF 数:        {pdfs_with_failures}");

    if total_checked == 0 {
        eprintln!("WARN: 未找到任何符合条件的 CJK span，测试无覆盖");
        return;
    }

    if total_failures > 0 {
        panic!(
            "CJK 字符宽度断言失败 — {total_failures} 个 span 为半角宽度（涉及 {pdfs_with_failures} 个 PDF），Identity-H 子集字体修复未生效"
        );
    }

    println!("\n========== 所有 PDF 的 CJK span 宽度均为全角范围内 ==========\n");
}
