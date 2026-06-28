use crate::ocr::OcrTextItem;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[cfg(feature = "pdfplumber")]
use pdfplumber::{Pdf, WordOptions, Word, BBox};
#[cfg(all(feature = "pdfplumber", test))]
use pdfplumber::TextDirection;
#[cfg(feature = "pdfplumber")]
use crate::ocr::engine::bbox_to_json;
#[cfg(feature = "pdfplumber")]
use crate::ocr::OcrPageResult;
// Column-aware merging: import column detection + per-column merge from layout_extractor.
// These are the correct primitives for multi-column Chinese invoices —
// merge_words_into_lines alone mixes buyer/seller columns when Y coordinates align.
#[cfg(feature = "pdfplumber")]
use crate::parser::layout_extractor::{detect_columns, merge_words_in_column};

/// PDF 文档类型分类
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PdfDocumentType {
    /// 发票（增值税电子发票、普通发票等）
    Invoice,
    /// 行程单（滴滴行程单、天府通电子行程单等）
    Itinerary,
    /// 结账单（酒店结账单等）
    Bill,
    /// 未知类型
    Unknown,
}

/// 根据文本内容判断 PDF 文档类型
/// 优先级：行程单 > 结账单 > 发票 > 未知
pub fn classify_pdf_document_type(text_items: &[OcrTextItem]) -> PdfDocumentType {
    let all_text: String = text_items
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    // 行程单检测
    if all_text.contains("行程单")
        || all_text.contains("行程报销单")
        || all_text.contains("电子行程单")
    {
        return PdfDocumentType::Itinerary;
    }

    // 结账单检测
    if all_text.contains("结账单") {
        return PdfDocumentType::Bill;
    }

    // 发票检测
    if all_text.contains("发票") || all_text.contains("增值税") {
        return PdfDocumentType::Invoice;
    }

    PdfDocumentType::Unknown
}

/// 从 PDF 文件中直接提取文字（适用于文字型 PDF）
/// 优先使用 parangi（Apache PDFBox 移植，完整 CJK 支持），
/// 失败时回退到 pdf-extract。
/// 返回提取到的文字行列表，如果 PDF 是扫描件（无可提取文字）则返回空 Vec
pub fn extract_text_from_pdf(file_path: &str) -> Result<Vec<OcrTextItem>, String> {
    let path = Path::new(file_path);

    // 优先使用 parangi（支持 UniGB-UCS2-H 等中文编码）
    match parangi::extract_text(path) {
        Ok(text) => {
            let items = text_to_items(&text);
            if !items.is_empty() {
                return Ok(items);
            }
            // parangi 返回空文本，尝试 pdf-extract
        }
        Err(e) => {
            eprintln!("  [parangi] 失败: {}, 尝试 pdf-extract...", e);
        }
    }

    // 回退到 pdf-extract（可能 panic，需要捕获）
    let result = std::panic::catch_unwind(|| pdf_extract::extract_text(file_path));

    let text = match result {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => return Err(format!("PDF 文字提取失败: {}", e)),
        Err(_) => return Err("PDF 文字提取失败: 不支持的编码或格式".to_string()),
    };

    Ok(text_to_items(&text))
}

fn text_to_items(text: &str) -> Vec<OcrTextItem> {
    text.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| OcrTextItem {
            text: line.to_string(),
            confidence: 1.0,
            box_coords: None,
        })
        .collect()
}

/// 判断提取到的文字是否足够用于发票解析
/// 当提取到的文字总长度超过阈值时，认为是文字型 PDF
pub fn has_sufficient_text(items: &[OcrTextItem], min_chars: usize) -> bool {
    let total_chars: usize = items.iter().map(|item| item.text.len()).sum();
    total_chars >= min_chars
}

// ──────────────────────────────────────────────
// CID 字体乱码检测
// ──────────────────────────────────────────────

/// 检测单个字符是否为 CID 字体乱码字符。
///
/// 乱码字符范围：
/// - 韩文字符 U+AC00..U+D7AF（CID 错误映射到韩文音节的典型表现）
/// - 韩文兼容字母 U+3130..U+318F
/// - PUA U+E000..U+F8FF
/// - 替换字符 U+FFFD
/// - 控制字符（排除空格 0x20、制表符 0x09、换行 0x0A、回车 0x0D）
/// - 代理项 U+D800..U+DFFF
fn is_garbled_char(c: char) -> bool {
    let code = c as u32;
    // 韩文音节（CID 错误映射的最常见表现）
    (code >= 0xAC00 && code <= 0xD7AF)
    // 韩文兼容字母
    || (code >= 0x3130 && code <= 0x318F)
    // PUA (Private Use Area)
    || (code >= 0xE000 && code <= 0xF8FF)
    // 替换字符
    || code == 0xFFFD
    // 控制字符（排除空格/换行/制表符）
    || (code < 0x20 && code != 0x09 && code != 0x0A && code != 0x0D)
    // 代理项
    || (code >= 0xD800 && code <= 0xDFFF)
}

/// 检测文本是否为 CID 字体乱码。
///
/// 乱码特征：
/// 1. `(cid:xxx)` 或 `CID:` 占位符 — 直接判定为乱码
/// 2. 大量韩文字符（U+AC00-U+D7AF）— CID 错误映射的典型表现
/// 3. PUA 字符（U+E000-U+F8FF）
/// 4. 替换字符 U+FFFD
///
/// # 判定规则
///
/// 当乱码字符占总字符数的比例 >= `threshold` 时返回 `true`。
/// 默认阈值 0.3（30%），可根据实际 PDF 样本调整。
///
/// # 示例
///
/// ```
/// use invoice_reimbursement_lib::pdf::text_extractor::is_garbled_text;
///
/// let garbled = "랢튻 욱뗧ퟓ랢욱ꎨ쳺슷뗧ퟓ뿍욱ꎩ춳";
/// assert!(is_garbled_text(garbled, 0.3));
///
/// let normal = "发票代码:043002200111湖南增值税电子普通发票";
/// assert!(!is_garbled_text(normal, 0.3));
/// ```
pub fn is_garbled_text(text: &str, threshold: f64) -> bool {
    // 1. 检测 (cid:xxx) 模式 — 直接判定乱码
    if text.contains("(cid:") || text.contains("CID:") {
        return true;
    }

    let total = text.chars().count();
    if total == 0 {
        return false;
    }

    let garbled = text.chars().filter(|&c| is_garbled_char(c)).count();
    (garbled as f64 / total as f64) >= threshold
}

/// 检测 `OcrTextItem` 列表的整体乱码率。
///
/// 将列表中所有文本拼接后调用 `is_garbled_text`，用于判断
/// pdfplumber 提取结果是否需要回退到 parangi。
pub fn is_garbled_items(items: &[OcrTextItem], threshold: f64) -> bool {
    let all_text: String = items
        .iter()
        .map(|i| i.text.as_str())
        .collect::<Vec<_>>()
        .join("");
    is_garbled_text(&all_text, threshold)
}

// ──────────────────────────────────────────────
// pdfplumber coordinate-aware text extraction
// ──────────────────────────────────────────────

/// Merge pdfplumber words into lines by Y-coordinate proximity.
///
/// Returns one entry per line with the joined text and merged bounding box.
#[cfg(feature = "pdfplumber")]
pub fn merge_words_into_lines(words: Vec<Word>) -> Vec<(String, BBox)> {
    if words.is_empty() {
        return Vec::new();
    }

    // Sort by Y (top), then by X (x0) for same-Y words
    let mut sorted = words;
    sorted.sort_by(|a, b| {
        a.bbox
            .top
            .partial_cmp(&b.bbox.top)
            .unwrap()
            .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
    });

    // Compute average word height; default to 12.0 if zero
    let avg_height = {
        let sum: f64 = sorted.iter().map(|w| w.bbox.height()).sum();
        let count = sorted.len() as f64;
        if count > 0.0 && sum > 0.0 {
            sum / count
        } else {
            12.0
        }
    };
    let y_tolerance = avg_height * 0.5;

    // X-gap threshold: if the horizontal gap between adjacent words (sorted by X)
    // exceeds this, they belong to different columns and should be separate items.
    // Using 2x average word height as the gap threshold — wide enough to join
    // normal word spacing, narrow enough to split multi-column layouts.
    let x_gap_threshold = avg_height * 2.0;

    // Group words into lines by Y-coordinate proximity
    let mut lines: Vec<Vec<&Word>> = Vec::new();
    for word in &sorted {
        if let Some(last_line) = lines.last() {
            let first_top = last_line[0].bbox.top;
            if (word.bbox.top - first_top).abs() > y_tolerance {
                lines.push(vec![word]);
            } else {
                lines.last_mut().unwrap().push(word);
            }
        } else {
            lines.push(vec![word]);
        }
    }

    // Convert each line group into one or more (String, BBox) items.
    // Within a line, split by X-gap: words with large horizontal gaps become
    // separate items (preserving multi-column layout structure).
    lines
        .into_iter()
        .flat_map(|line_words| {
            if line_words.is_empty() {
                return Vec::new();
            }

            // line_words is already sorted by X (from the initial sort)
            let mut groups: Vec<Vec<&Word>> = vec![vec![line_words[0]]];
            for &word in &line_words[1..] {
                let prev = groups.last().unwrap().last().unwrap();
                let gap = word.bbox.x0 - prev.bbox.x1;
                if gap > x_gap_threshold {
                    // Large gap → new column item
                    groups.push(vec![word]);
                } else {
                    // Small gap → same column, join
                    groups.last_mut().unwrap().push(word);
                }
            }

            // Convert each X-group into (String, BBox)
            groups
                .into_iter()
                .map(|group_words| {
                    let text = group_words
                        .iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let x0 = group_words
                        .iter()
                        .map(|w| w.bbox.x0)
                        .fold(f64::INFINITY, f64::min);
                    let top = group_words
                        .iter()
                        .map(|w| w.bbox.top)
                        .fold(f64::INFINITY, f64::min);
                    let x1 = group_words
                        .iter()
                        .map(|w| w.bbox.x1)
                        .fold(f64::NEG_INFINITY, f64::max);
                    let bottom = group_words
                        .iter()
                        .map(|w| w.bbox.bottom)
                        .fold(f64::NEG_INFINITY, f64::max);
                    (text, BBox::new(x0, top, x1, bottom))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Extract text from a PDF file with coordinate information using pdfplumber.
///
/// Returns per-page results similar to OCR output, suitable for the existing
/// pipeline that expects `Vec<OcrPageResult>`.
#[cfg(feature = "pdfplumber")]
pub fn extract_text_with_coords(file_path: &str) -> Result<Vec<OcrPageResult>, String> {
    // Wrap pdfplumber in catch_unwind — it may panic on non-standard PDFs
    // (CID font parsing, encrypted PDFs, etc.). Same pattern as pdf-extract.
    let result = std::panic::catch_unwind(|| {
        extract_text_with_coords_inner(file_path)
    });
    match result {
        Ok(inner) => inner,
        Err(panic_msg) => {
            let msg = if let Some(s) = panic_msg.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_msg.downcast_ref::<String>() {
                s.clone()
            } else {
                "pdfplumber panicked (unknown cause)".to_string()
            };
            eprintln!("  [pdfplumber] panic: {}", msg);
            Err(format!("pdfplumber panic: {}", msg))
        }
    }
}

#[cfg(feature = "pdfplumber")]
fn extract_text_with_coords_inner(file_path: &str) -> Result<Vec<OcrPageResult>, String> {
    let pdf = Pdf::open_file(file_path, None).map_err(|e| format!("pdfplumber: {}", e))?;

    let mut results: Vec<OcrPageResult> = Vec::new();
    let mut total_words: usize = 0;

    for page_result in pdf.pages_iter() {
        let page = page_result.map_err(|e| format!("pdfplumber page: {}", e))?;
        let words = page.extract_words(&WordOptions::default());
        total_words += words.len();

        // 列感知合并：多栏发票不会跨列混合买方/卖方
        let lines = column_aware_merge(words);
        let texts: Vec<OcrTextItem> = lines
            .into_iter()
            .map(|(text, bbox)| OcrTextItem {
                text,
                confidence: 1.0,
                box_coords: Some(bbox_to_json(bbox.x0, bbox.top, bbox.x1, bbox.bottom, 1.0)),
            })
            .collect();

        results.push(OcrPageResult {
            page: page.page_number() as u32,
            texts,
        });
    }

    if results.is_empty() || total_words == 0 {
        return Err("pdfplumber extracted no text".to_string());
    }

    Ok(results)
}

/// Convenience wrapper that flattens `extract_text_with_coords` into a single
/// Vec of `OcrTextItem` (all pages combined).
#[cfg(feature = "pdfplumber")]
pub fn extract_text_with_coords_flat(file_path: &str) -> Result<Vec<OcrTextItem>, String> {
    let pages = extract_text_with_coords(file_path)?;
    Ok(pages.into_iter().flat_map(|p| p.texts).collect())
}

/// Debug: 返回 pdfplumber 原始 Word 级数据（未经 merge_words_into_lines 合并），
/// 用于验证分栏检测可行性。每项 = (text, x0, top, x1, bottom, page_number)
#[cfg(feature = "pdfplumber")]
pub fn extract_raw_words_debug(
    file_path: &str,
) -> Result<Vec<(String, f64, f64, f64, f64, u32)>, String> {
    let result = std::panic::catch_unwind(|| {
        let pdf = Pdf::open_file(file_path, None).map_err(|e| format!("pdfplumber: {}", e))?;
        let mut out: Vec<(String, f64, f64, f64, f64, u32)> = Vec::new();
        for page_result in pdf.pages_iter() {
            let page = page_result.map_err(|e| format!("pdfplumber page: {}", e))?;
            let pn = page.page_number() as u32;
            let words = page.extract_words(&WordOptions::default());
            for w in &words {
                out.push((
                    w.text.clone(),
                    w.bbox.x0,
                    w.bbox.top,
                    w.bbox.x1,
                    w.bbox.bottom,
                    pn,
                ));
            }
        }
        Ok(out)
    });
    match result {
        Ok(inner) => inner,
        Err(_) => Err("pdfplumber panic in extract_raw_words_debug".to_string()),
    }
}

/// 从 PDF 提取 pdfplumber 原始 Word 列表（未经 merge_words_into_lines 合并），
/// 保留每个 Word 的完整坐标信息，供 layout_extractor 做坐标分栏/分框。
#[cfg(feature = "pdfplumber")]
pub fn extract_words_raw(file_path: &str) -> Result<Vec<Word>, String> {
    let result = std::panic::catch_unwind(|| {
        let pdf = Pdf::open_file(file_path, None).map_err(|e| format!("pdfplumber: {}", e))?;
        let mut all_words: Vec<Word> = Vec::new();
        for page_result in pdf.pages_iter() {
            let page = page_result.map_err(|e| format!("pdfplumber page: {}", e))?;
            let words = page.extract_words(&WordOptions::default());
            all_words.extend(words);
        }
        if all_words.is_empty() {
            return Err("pdfplumber extracted no words".to_string());
        }
        Ok(all_words)
    });
    match result {
        Ok(inner) => inner,
        Err(_) => Err("pdfplumber panic in extract_words_raw".to_string()),
    }
}

// ──────────────────────────────────────────────
// Column-aware extraction (P1: fixes multi-column merging)
// ──────────────────────────────────────────────

/// 单次 PDF 打开的完整提取结果，包含合并后的行文本和原始 Word 列表。
///
/// 解决两个问题：
/// 1. **列感知合并**：多栏发票（买方/卖方）不会跨列合并
/// 2. **性能**：管线不再需要二次打开 PDF 获取 raw words
#[cfg(feature = "pdfplumber")]
pub struct PdfExtraction {
    /// 按页组织的合并后文本项（列感知合并，可直接用于正则解析）
    pub pages: Vec<OcrPageResult>,
    /// 全部原始 Word（未经合并，保留完整坐标，供坐标提取器使用）
    pub raw_words: Vec<Word>,
}

/// 列感知合并：检测列布局，在每列内独立合并 Word 为行。
///
/// - 单栏：退化为 `merge_words_into_lines`（向后兼容）
/// - 多栏：用 `detect_columns` 检测列边界，每列内用 `merge_words_in_column` 合并
///
/// 这解决了 `merge_words_into_lines` 将买方/卖方列合并到同一行的根本问题。
///
/// 容差参数从 `LayoutTuning::default()` 获取，集中管理硬编码值。
#[cfg(feature = "pdfplumber")]
fn column_aware_merge(words: Vec<Word>) -> Vec<(String, BBox)> {
    if words.is_empty() {
        return Vec::new();
    }

    // 使用 LayoutTuning 集中管理容差参数
    let tuning = crate::parser::layout_extractor::LayoutTuning::default();
    let avg_height = crate::parser::layout_extractor::LayoutTuning::avg_height_of(&words);
    let y_tolerance = tuning.y_tolerance(avg_height);
    let x_gap_threshold = tuning.x_gap_threshold(avg_height);

    // 检测列布局
    let layout = detect_columns(&words);

    if layout.columns.is_empty() || layout.is_single_column() {
        // 单栏或无法检测：使用原有合并逻辑
        merge_words_into_lines(words)
    } else {
        // 多栏：每列内独立合并，避免跨列混合
        let mut all_lines: Vec<(String, BBox)> = Vec::new();
        for column in &layout.columns {
            let col_words: Vec<&Word> = words
                .iter()
                .filter(|w| {
                    let cx = (w.bbox.x0 + w.bbox.x1) / 2.0;
                    cx >= column.x_min && cx <= column.x_max
                })
                .collect();
            if col_words.is_empty() {
                continue;
            }
            let col_lines = merge_words_in_column(&col_words, y_tolerance, x_gap_threshold);
            all_lines.extend(col_lines);
        }
        // 按 Y 再按 X 排序，保持阅读顺序
        all_lines.sort_by(|a, b| {
            a.1.top
                .partial_cmp(&b.1.top)
                .unwrap()
                .then(a.1.x0.partial_cmp(&b.1.x0).unwrap())
        });
        all_lines
    }
}

/// 单次打开 PDF，返回列感知合并的文本项 + 原始 Word 列表。
///
/// 替代 `extract_text_with_coords` + `extract_words_raw` 的两次 PDF 打开，
/// 同时通过列感知合并修复多栏发票的买方/卖方混合问题。
#[cfg(feature = "pdfplumber")]
pub fn extract_pdf_column_aware(file_path: &str) -> Result<PdfExtraction, String> {
    let result = std::panic::catch_unwind(|| {
        let pdf = Pdf::open_file(file_path, None).map_err(|e| format!("pdfplumber: {}", e))?;

        let mut pages: Vec<OcrPageResult> = Vec::new();
        let mut all_words: Vec<Word> = Vec::new();
        let mut total_words: usize = 0;

        for page_result in pdf.pages_iter() {
            let page = page_result.map_err(|e| format!("pdfplumber page: {}", e))?;
            let words = page.extract_words(&WordOptions::default());
            total_words += words.len();

            // 克隆一份用于坐标提取器（column_aware_merge 消费原始 Vec）
            all_words.extend(words.clone());

            // 列感知合并
            let lines = column_aware_merge(words);

            let texts: Vec<OcrTextItem> = lines
                .into_iter()
                .map(|(text, bbox)| OcrTextItem {
                    text,
                    confidence: 1.0,
                    box_coords: Some(bbox_to_json(
                        bbox.x0, bbox.top, bbox.x1, bbox.bottom, 1.0,
                    )),
                })
                .collect();

            pages.push(OcrPageResult {
                page: page.page_number() as u32,
                texts,
            });
        }

        if pages.is_empty() || total_words == 0 {
            return Err("pdfplumber extracted no text".to_string());
        }

        Ok(PdfExtraction {
            pages,
            raw_words: all_words,
        })
    });
    match result {
        Ok(inner) => inner,
        Err(panic_msg) => {
            let msg = if let Some(s) = panic_msg.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_msg.downcast_ref::<String>() {
                s.clone()
            } else {
                "pdfplumber panicked (unknown cause)".to_string()
            };
            eprintln!("  [pdfplumber] panic: {}", msg);
            Err(format!("pdfplumber panic: {}", msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_sufficient_text_empty() {
        let items = vec![];
        assert!(!has_sufficient_text(&items, 10));
    }

    #[test]
    fn test_has_sufficient_text_short() {
        let items = vec![OcrTextItem {
            text: "发票".to_string(),
            confidence: 1.0,
            box_coords: None,
        }];
        assert!(!has_sufficient_text(&items, 10));
    }

    #[test]
    fn test_has_sufficient_text_enough() {
        let items = vec![
            OcrTextItem {
                text: "增值税电子普通发票".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
            OcrTextItem {
                text: "发票代码：12345678".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
            OcrTextItem {
                text: "发票号码：87654321".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
            OcrTextItem {
                text: "开票日期：2025年06月15日".to_string(),
                confidence: 1.0,
                box_coords: None,
            },
        ];
        assert!(has_sufficient_text(&items, 10));
    }

    #[test]
    fn test_classify_invoice() {
        let items = vec![
            OcrTextItem { text: "增值税电子普通发票".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "发票号码：12345678".to_string(), confidence: 1.0, box_coords: None },
        ];
        assert_eq!(classify_pdf_document_type(&items), PdfDocumentType::Invoice);
    }

    #[test]
    fn test_classify_itinerary() {
        let items = vec![
            OcrTextItem { text: "滴滴出行行程单".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "出发时间：2025-01-01".to_string(), confidence: 1.0, box_coords: None },
        ];
        assert_eq!(classify_pdf_document_type(&items), PdfDocumentType::Itinerary);
    }

    #[test]
    fn test_classify_itinerary_transit() {
        let items = vec![
            OcrTextItem { text: "天府通电子行程单".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "地铁消费".to_string(), confidence: 1.0, box_coords: None },
        ];
        assert_eq!(classify_pdf_document_type(&items), PdfDocumentType::Itinerary);
    }

    #[test]
    fn test_classify_bill() {
        let items = vec![
            OcrTextItem { text: "成都九眼桥美居酒店结账单".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "住宿费".to_string(), confidence: 1.0, box_coords: None },
        ];
        assert_eq!(classify_pdf_document_type(&items), PdfDocumentType::Bill);
    }

    #[test]
    fn test_classify_unknown() {
        let items = vec![
            OcrTextItem { text: "普通收据".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "金额：50元".to_string(), confidence: 1.0, box_coords: None },
        ];
        assert_eq!(classify_pdf_document_type(&items), PdfDocumentType::Unknown);
    }

    // ── pdfplumber merge_words_into_lines tests ──
    #[cfg(feature = "pdfplumber")]
    #[test]
    fn test_merge_words_into_lines_basic() {
        let words = vec![
            Word {
                text: "Hello".to_string(),
                bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
                doctop: 100.0,
                direction: TextDirection::Ltr,
                chars: vec![],
            },
            Word {
                text: "World".to_string(),
                bbox: BBox::new(55.0, 100.0, 95.0, 112.0),
                doctop: 100.0,
                direction: TextDirection::Ltr,
                chars: vec![],
            },
            Word {
                text: "Test".to_string(),
                bbox: BBox::new(100.0, 100.0, 130.0, 112.0),
                doctop: 100.0,
                direction: TextDirection::Ltr,
                chars: vec![],
            },
        ];

        let lines = merge_words_into_lines(words);
        assert_eq!(lines.len(), 1);

        let (text, bbox) = &lines[0];
        assert_eq!(text, "Hello World Test");
        assert!((bbox.x0 - 10.0).abs() < 1e-6);
        assert!((bbox.top - 100.0).abs() < 1e-6);
        assert!((bbox.x1 - 130.0).abs() < 1e-6);
        assert!((bbox.bottom - 112.0).abs() < 1e-6);
    }

    #[cfg(feature = "pdfplumber")]
    #[test]
    fn test_merge_words_into_lines_multiple_lines() {
        let words = vec![
            // Line 1 (Y = 100)
            Word {
                text: "First".to_string(),
                bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
                doctop: 100.0,
                direction: TextDirection::Ltr,
                chars: vec![],
            },
            Word {
                text: "Line".to_string(),
                bbox: BBox::new(55.0, 100.0, 85.0, 112.0),
                doctop: 100.0,
                direction: TextDirection::Ltr,
                chars: vec![],
            },
            // Line 2 (Y = 150 — 50px below, well beyond 0.5 * avg_height)
            Word {
                text: "Second".to_string(),
                bbox: BBox::new(10.0, 150.0, 60.0, 162.0),
                doctop: 150.0,
                direction: TextDirection::Ltr,
                chars: vec![],
            },
            Word {
                text: "Line".to_string(),
                bbox: BBox::new(65.0, 150.0, 95.0, 162.0),
                doctop: 150.0,
                direction: TextDirection::Ltr,
                chars: vec![],
            },
            Word {
                text: "Too".to_string(),
                bbox: BBox::new(100.0, 150.0, 125.0, 162.0),
                doctop: 150.0,
                direction: TextDirection::Ltr,
                chars: vec![],
            },
            // Line 3 (Y = 50 — 50px above, well beyond tolerance)
            Word {
                text: "Top".to_string(),
                bbox: BBox::new(10.0, 50.0, 40.0, 62.0),
                doctop: 50.0,
                direction: TextDirection::Ltr,
                chars: vec![],
            },
        ];

        let lines = merge_words_into_lines(words);
        assert_eq!(lines.len(), 3);

        // Lines should be sorted by Y: Top (50), First Line (100), Second Line (150)
        assert_eq!(lines[0].0, "Top");
        assert_eq!(lines[1].0, "First Line");
        assert_eq!(lines[2].0, "Second Line Too");
    }

    #[cfg(feature = "pdfplumber")]
    #[test]
    fn test_merge_words_into_lines_empty() {
        let words: Vec<Word> = vec![];
        let lines = merge_words_into_lines(words);
        assert!(lines.is_empty());
    }

    // ── column_aware_merge tests (P1: multi-column invoice fix) ──

    #[cfg(feature = "pdfplumber")]
    fn make_word(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Word {
        Word {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            doctop: top,
            direction: TextDirection::Ltr,
            chars: vec![],
        }
    }

    #[cfg(feature = "pdfplumber")]
    #[test]
    fn test_column_aware_merge_multi_column_does_not_mix() {
        // 模拟增值税电子发票双栏布局：
        //   左栏（买方）: X=31-175, Y=95
        //   右栏（卖方）: X=301-450, Y=95
        // 旧 merge_words_into_lines 会合并为一行 "买方 卖方"
        // column_aware_merge 应检测到双栏，各自独立合并
        let words = vec![
            // 表头行（Y < 80 被排除列检测，但合并仍处理）
            make_word("发票", 100.0, 30.0, 130.0, 42.0),
            // 买方名称（左栏）
            make_word("名称：中国人民解放军国防科技大学", 31.0, 95.0, 175.0, 104.0),
            // 卖方名称（右栏，同 Y 行）
            make_word("名称：成都滴滴优行科技有限公司", 301.0, 95.0, 450.0, 104.0),
            // 买方税号（左栏，下一行）
            make_word("纳税人识别号：91110108A1100000M", 31.0, 120.0, 200.0, 129.0),
            // 卖方税号（右栏，同 Y 行）
            make_word("纳税人识别号：91430100578607044B", 301.0, 120.0, 450.0, 129.0),
        ];

        let lines = column_aware_merge(words);

        // 关键断言：买方和卖方不应出现在同一行
        let has_mixed = lines.iter().any(|(text, _)| {
            text.contains("国防") && text.contains("滴滴")
        });
        assert!(
            !has_mixed,
            "多栏合并不应将买方和卖方混合到同一行: {:?}",
            lines.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>()
        );

        // 应该至少有 2 个包含"名称"的行（买方和卖方各一个）
        let name_lines: Vec<_> = lines.iter().filter(|(t, _)| t.contains("名称")).collect();
        assert!(
            name_lines.len() >= 2,
            "应有至少2个名称行（买方+卖方），实际 {}: {:?}",
            name_lines.len(),
            name_lines.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>()
        );
    }

    #[cfg(feature = "pdfplumber")]
    #[test]
    fn test_column_aware_merge_single_column_preserves_behavior() {
        // 单栏文档：column_aware_merge 应退化为 merge_words_into_lines 的行为
        // 使用紧密排列的单词（小间距），确保 detect_columns 判定为单栏
        let words = vec![
            make_word("Hello", 10.0, 100.0, 50.0, 112.0),
            make_word("World", 52.0, 100.0, 92.0, 112.0),
            make_word("Test", 94.0, 100.0, 124.0, 112.0),
            // 添加更多行确保不是单行偶然通过
            make_word("Second", 10.0, 120.0, 60.0, 132.0),
            make_word("Line", 62.0, 120.0, 92.0, 132.0),
        ];

        let lines = column_aware_merge(words);
        // 单栏应合并为 2 行（Y=100 和 Y=120）
        assert_eq!(lines.len(), 2, "单栏应合并为2行");
        assert_eq!(lines[0].0, "Hello World Test");
        assert_eq!(lines[1].0, "Second Line");
    }

    #[cfg(feature = "pdfplumber")]
    #[test]
    fn test_column_aware_merge_empty() {
        let words: Vec<Word> = vec![];
        let lines = column_aware_merge(words);
        assert!(lines.is_empty());
    }

    // ── CID 字体乱码检测 ──

    #[test]
    fn test_is_garbled_text_korean_cid_garble() {
        // #2 铁路电子客票的真实乱码样本
        let garbled = "랢튻 욱뗧ퟓ랢욱ꎨ쳺슷뗧ퟓ뿍욱ꎩ춳 맺볒쮰컱ퟜ뻖";
        assert!(is_garbled_text(garbled, 0.3));
    }

    #[test]
    fn test_is_garbled_text_normal_chinese() {
        let normal = "发票代码:043002200111湖南增值税电子普通发票 名称：成都滴滴优行科技有限公司";
        assert!(!is_garbled_text(normal, 0.3));
    }

    #[test]
    fn test_is_garbled_text_cid_placeholder() {
        assert!(is_garbled_text("hello (cid:123) world", 0.3));
    }

    #[test]
    fn test_is_garbled_text_pua_chars() {
        // PUA 字符，占比需 > 30%
        // 4 个 PUA 字符 + " text" = 10 chars → 40% > 30%
        let pua = "\u{E000}\u{E001}\u{E002}\u{E003} text";
        assert!(is_garbled_text(pua, 0.3));
    }

    #[test]
    fn test_is_garbled_text_mixed_low_ratio() {
        // 少量乱码字符（低于阈值）不应判定为乱码
        let mixed = "发票号码:32092584 \u{E000} 正常文本";
        assert!(!is_garbled_text(mixed, 0.3));
    }

    #[test]
    fn test_is_garbled_text_empty() {
        assert!(!is_garbled_text("", 0.3));
    }

    #[test]
    fn test_is_garbled_items_detects_garble() {
        let items = vec![
            OcrTextItem { text: "랢튻 욱뗧ퟓ랢욱".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "맺볒쮰컱ퟜ뻖".to_string(), confidence: 1.0, box_coords: None },
        ];
        assert!(is_garbled_items(&items, 0.3));
    }

    #[test]
    fn test_is_garbled_items_normal() {
        let items = vec![
            OcrTextItem { text: "发票代码:043002200111".to_string(), confidence: 1.0, box_coords: None },
            OcrTextItem { text: "名称：成都滴滴优行科技有限公司".to_string(), confidence: 1.0, box_coords: None },
        ];
        assert!(!is_garbled_items(&items, 0.3));
    }
}
