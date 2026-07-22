use crate::ocr::OcrTextItem;
use serde::{Deserialize, Serialize};

#[cfg(feature = "pdfplumber")]
use pdfplumber::{Pdf, WordOptions, Word, BBox, TableSettings};
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
    // (CID font parsing, encrypted PDFs, etc.).
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
        // ponytail: 读字节后用 Pdf::open（与 zpdf 同路径），open 失败回退 open_with_repair
        // ——zpdf 容忍的 broken xref/stream length，lopdf 会拒绝，repair 专治此症
        let bytes = std::fs::read(file_path).map_err(|e| format!("pdfplumber read: {}", e))?;
        let pdf = match Pdf::open(&bytes, None) {
            Ok(p) => p,
            Err(e) => {
                let (p, _) = Pdf::open_with_repair(&bytes, None, None)
                    .map_err(|e2| format!("pdfplumber open 失败: {e}; repair 也失败: {e2}"))?;
                p
            }
        };
        let mut out: Vec<(String, f64, f64, f64, f64, u32)> = Vec::new();
        let mut page_errors: Vec<String> = Vec::new();
        for page_result in pdf.pages_iter() {
            // ponytail: 逐页容错——pages_iter 每页独立 Result，一页坏不该丢掉后续好页
            let page = match page_result {
                Ok(p) => p,
                Err(e) => {
                    page_errors.push(format!("pdfplumber page: {e}"));
                    continue;
                }
            };
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
        if out.is_empty() {
            let mut msg = if page_errors.is_empty() {
                "pdfplumber 打开成功但 extract_words 返回 0 words（疑似 CID 字体无 ToUnicode）"
                    .to_string()
            } else {
                page_errors.join("; ")
            };
            // ponytail: 0.2.0 tokenizer 遇 << 硬失败（PR#214 已修但未发布），提示用户
            if msg.contains("unexpected '<<'") {
                msg.push_str("（pdfplumber-rs 0.2.0 已知限制：tokenizer 拒绝内容流中的 <<，上游 PR#214 已修但未发版）");
            }
            return Err(msg);
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

/// 单元格内单个 word 的坐标信息（Type 1 数据的元素）
#[cfg(feature = "pdfplumber")]
#[derive(Debug, Clone, Default)]
pub struct WordInCell {
    pub text: String,
    pub x0: f64,
    pub top: f64,
    pub x1: f64,
    pub bottom: f64,
}

/// pdfplumber find_tables() 返回的单元格信息（文本 + 坐标）
///
/// 含 4 类文本数据，按提取场景选用：
/// - `text`: pdfplumber 原始合并文本（向后兼容）
/// - `words`: Type 1，原始 word 数组（每 word 独立坐标）
/// - `line_text`: Type 2，按行组装（word 按 Y 分组，行间 \n）— 适用横排标签值
/// - `merged_text`: Type 3，全部合并（去除空白）— 适用竖排标签 / 小单元格
/// - `column_text`: Type 4，按列聚合（word 按 X 分组）— 适用商品详情大单元格
#[cfg(feature = "pdfplumber")]
#[derive(Debug, Clone, Default)]
pub struct TableCellInfo {
    pub text: String,
    pub x0: f64,
    pub top: f64,
    pub x1: f64,
    pub bottom: f64,
    /// Type 1: 原始 word 数组
    pub words: Vec<WordInCell>,
    /// Type 2: 按行组装文本
    pub line_text: String,
    /// Type 3: 全部合并文本（去空白）
    pub merged_text: String,
    /// Type 4: 按列聚合文本
    pub column_text: String,
}

/// pdfplumber find_tables() 返回的表格信息（行 × 单元格）
#[cfg(feature = "pdfplumber")]
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub rows: Vec<Vec<TableCellInfo>>,
    pub x0: f64,
    pub top: f64,
    pub x1: f64,
    pub bottom: f64,
}

/// 单次 PDF 打开的完整提取结果，包含合并后的行文本和原始 Word 列表。
///
/// 解决两个问题：
/// 1. **列感知合并**：多栏发票（买方/卖方）不会跨列合并
/// 2. **性能**：管线不再需要二次打开 PDF 获取 raw words
#[cfg(feature = "pdfplumber")]
pub struct PdfExtraction {
    /// 按页组织的合并后文本项（列感知合并，可直接用于正则解析）
    pub pages: Vec<OcrPageResult>,
    /// 按页组织的 word 级文本项（未合并，保留独立坐标，供行程单表格解析）
    pub word_pages: Vec<OcrPageResult>,
    /// 全部原始 Word（未经合并，保留完整坐标，供坐标提取器使用）
    pub raw_words: Vec<Word>,
    /// 按页组织的表格（find_tables 结果，供单元格引导提取使用）
    pub tables: Vec<Vec<TableInfo>>,
}

/// 用页面 Word 填充每个单元格的 4 类文本数据（words/line_text/merged_text/column_text）。
///
/// 关联方式：word 中心点 (cx, cy) 落在 cell 边界内则属于该 cell。
/// 一个 word 可能落进多个重叠 cell（pdfplumber 表格常有共享边框），
/// 取面积包含的 cell（通常唯一）。
#[cfg(feature = "pdfplumber")]
fn enrich_cells_with_words(tables: &mut [TableInfo], words: &[Word]) {
    for table in tables.iter_mut() {
        for row in &mut table.rows {
            for cell in row.iter_mut() {
                let cell_words: Vec<&Word> = words
                    .iter()
                    .filter(|w| {
                        let cx = (w.bbox.x0 + w.bbox.x1) / 2.0;
                        let cy = (w.bbox.top + w.bbox.bottom) / 2.0;
                        cx >= cell.x0 && cx <= cell.x1 && cy >= cell.top && cy <= cell.bottom
                    })
                    .collect();
                if cell_words.is_empty() {
                    continue;
                }
                // Type 1: words 数组
                cell.words = cell_words
                    .iter()
                    .map(|w| WordInCell {
                        text: w.text.clone(),
                        x0: w.bbox.x0,
                        top: w.bbox.top,
                        x1: w.bbox.x1,
                        bottom: w.bbox.bottom,
                    })
                    .collect();
                // Type 2: 按行组装
                cell.line_text = build_line_text_from_words(&cell_words);
                // Type 3: 去除所有空白
                cell.merged_text = cell.line_text.chars().filter(|c| !c.is_whitespace()).collect();
                // Type 4: 按列聚合
                cell.column_text = build_column_text_from_words(&cell_words);
            }
        }
    }
}

/// Type 2: word 按 Y 分组为行，每行内按 X 排序，word 间用空格连接，行间用 \n。
#[cfg(feature = "pdfplumber")]
fn build_line_text_from_words(words: &[&Word]) -> String {
    if words.is_empty() {
        return String::new();
    }
    let mut sorted: Vec<&Word> = words.iter().copied().collect();
    let avg_h = sorted.iter().map(|w| w.bbox.bottom - w.bbox.top).sum::<f64>() / sorted.len() as f64;
    let y_tol = (avg_h * 0.5).max(2.0);
    sorted.sort_by(|a, b| {
        a.bbox.top.partial_cmp(&b.bbox.top).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut lines: Vec<Vec<&Word>> = Vec::new();
    for w in sorted {
        let cy = (w.bbox.top + w.bbox.bottom) / 2.0;
        if let Some(last) = lines.last_mut() {
            let last_cy = (last[0].bbox.top + last[0].bbox.bottom) / 2.0;
            if (cy - last_cy).abs() <= y_tol {
                last.push(w);
                continue;
            }
        }
        lines.push(vec![w]);
    }
    lines
        .iter()
        .map(|line| line.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Type 4: word 按 X 分组为列，每列内按 Y 排序，word 间直接连接（CID 间距），
/// 列间用空格分隔。适用于商品详情等大单元格内按列布置的文本。
#[cfg(feature = "pdfplumber")]
fn build_column_text_from_words(words: &[&Word]) -> String {
    if words.is_empty() {
        return String::new();
    }
    let mut sorted: Vec<&Word> = words.iter().copied().collect();
    let avg_w = sorted.iter().map(|w| w.bbox.x1 - w.bbox.x0).sum::<f64>() / sorted.len() as f64;
    let x_tol = (avg_w * 0.5).max(2.0);
    sorted.sort_by(|a, b| {
        a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.bbox.top.partial_cmp(&b.bbox.top).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut cols: Vec<Vec<&Word>> = Vec::new();
    for w in sorted {
        let cx = (w.bbox.x0 + w.bbox.x1) / 2.0;
        if let Some(last) = cols.last_mut() {
            let last_cx = (last[0].bbox.x0 + last[0].bbox.x1) / 2.0;
            if (cx - last_cx).abs() <= x_tol {
                last.push(w);
                continue;
            }
        }
        cols.push(vec![w]);
    }
    cols.iter()
        .map(|col| col.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(""))
        .collect::<Vec<_>>()
        .join(" ")
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

/// 对指定 bbox 内的 words 做列感知合并，返回合并后的完整文本（用于从商品详情格提取商家自定义名称）
#[cfg(feature = "pdfplumber")]
pub fn column_aware_merge_in_bbox(words: &[pdfplumber::Word], x0: f64, top: f64, x1: f64, bottom: f64) -> String {
    let within: Vec<pdfplumber::Word> = words.iter()
        .filter(|w| {
            let cx = (w.bbox.x0 + w.bbox.x1) / 2.0;
            let cy = (w.bbox.top + w.bbox.bottom) / 2.0;
            cx >= x0 && cx <= x1 && cy >= top && cy <= bottom
        })
        .cloned()
        .collect();
    let lines = column_aware_merge(within);
    lines.into_iter()
        .map(|(text, _)| text)
        .collect::<Vec<_>>()
        .join(" ")
}

/// 单次 PDF 打开：列感知合并 + 原始 Word（供坐标提取器使用，无需二次打开）
#[cfg(feature = "pdfplumber")]
pub fn extract_pdf_column_aware(file_path: &str) -> Result<PdfExtraction, String> {
    match extract_pdfplumber_column_aware(file_path) {
        Ok(extraction) => Ok(extraction),
        Err(plumber_err) => {
            eprintln!("  [pdfplumber] 失败: {}，回退到 parangi/OCR", plumber_err);
            Err(plumber_err)
        }
    }
}

/// pdfplumber 列感知提取（原 extract_pdf_column_aware 逻辑）
fn extract_pdfplumber_column_aware(file_path: &str) -> Result<PdfExtraction, String> {
    let result = std::panic::catch_unwind(|| {
        let pdf = Pdf::open_file(file_path, None).map_err(|e| format!("pdfplumber: {}", e))?;

        let mut pages: Vec<OcrPageResult> = Vec::new();
        let mut word_pages: Vec<OcrPageResult> = Vec::new();
        let mut all_words: Vec<Word> = Vec::new();
        let mut all_tables: Vec<Vec<TableInfo>> = Vec::new();
        let mut total_words: usize = 0;

        for page_result in pdf.pages_iter() {
            let page = page_result.map_err(|e| format!("pdfplumber page: {}", e))?;
            let page_number = page.page_number() as u32;
            let words = page.extract_words(&WordOptions::default());
            total_words += words.len();

            // 克隆一份用于坐标提取器（column_aware_merge 消费原始 Vec）
            all_words.extend(words.clone());

            // 表格单元格提取（find_tables 填充单元格文本，供 cell_extractor 使用）
            let tables = page.find_tables(&TableSettings::default());
            let mut table_infos: Vec<TableInfo> = tables
                .iter()
                .map(|t| {
                    let rows: Vec<Vec<TableCellInfo>> = t
                        .rows
                        .iter()
                        .map(|row| {
                            row.iter()
                                .map(|c| TableCellInfo {
                                    text: c.text.clone().unwrap_or_default(),
                                    x0: c.bbox.x0,
                                    top: c.bbox.top,
                                    x1: c.bbox.x1,
                                    bottom: c.bbox.bottom,
                                    ..Default::default()
                                })
                                .collect()
                        })
                        .collect();
                    TableInfo {
                        rows,
                        x0: t.bbox.x0,
                        top: t.bbox.top,
                        x1: t.bbox.x1,
                        bottom: t.bbox.bottom,
                    }
                })
                .collect();
            enrich_cells_with_words(&mut table_infos, &words);
            all_tables.push(table_infos);

            // word 级文本项（未合并，保留独立坐标，供行程单表格解析）
            let word_texts: Vec<OcrTextItem> = words.iter().map(|w| OcrTextItem {
                text: w.text.clone(),
                confidence: 1.0,
                box_coords: Some(bbox_to_json(w.bbox.x0, w.bbox.top, w.bbox.x1, w.bbox.bottom, 1.0)),
            }).collect();
            word_pages.push(OcrPageResult { page: page_number, texts: word_texts });

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
                page: page_number,
                texts,
            });
        }

        if pages.is_empty() || total_words == 0 {
            return Err("pdfplumber extracted no text".to_string());
        }

        Ok(PdfExtraction {
            pages,
            word_pages,
            raw_words: all_words,
            tables: all_tables,
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

/// 从 pdfplumber Word 的 char 列表重建行文本。
///
/// 核心逻辑：
/// 1. 收集所有 Word 的 chars
/// 2. 按 Y 坐标分行（容差 = 平均行高 × 0.5）
/// 3. 行内按 X 坐标排序
/// 4. **间距过滤**：相邻 char 间距 < avg_char_width × 0.5 时跳过当前 char
///    （pdfplumber word 拆分会产生位置重叠的噪声 char，间距过滤可去除）
/// 5. 拼接剩余 char 的 text
///
/// 解决问题：火车票 "2025年11月14日 5:22开" 被 pdfplumber 拆成
/// "2025年11月1" + "1"(噪声@x=124.7) + "4" + "5日:22开"，
/// 噪声 "1" 插在 day 十位和个位之间破坏正则匹配。
#[cfg(feature = "pdfplumber")]
pub fn reconstruct_lines_from_chars(words: &[Word]) -> Vec<String> {
    use pdfplumber::Char;

    // 收集所有 chars (text, x0, y0)
    let mut chars: Vec<(&Char, f64, f64)> = words.iter()
        .flat_map(|w| w.chars.iter())
        .map(|c| (c, c.bbox.x0, c.bbox.top))
        .collect();
    if chars.is_empty() {
        return Vec::new();
    }

    // 计算平均 char 宽度（用于间距过滤阈值）
    let widths: Vec<f64> = chars.iter()
        .map(|(c, _, _)| c.bbox.x1 - c.bbox.x0)
        .filter(|w| *w > 0.0)
        .collect();
    let avg_width = if widths.is_empty() {
        12.0
    } else {
        widths.iter().sum::<f64>() / widths.len() as f64
    };
    let gap_threshold = avg_width * 0.5; // 间距 < 半个 char 宽 = 噪声

    // 计算平均行高用于 Y 分行
    let heights: Vec<f64> = chars.iter()
        .map(|(c, _, _)| c.bbox.height())
        .filter(|h| *h > 0.0)
        .collect();
    let avg_height = if heights.is_empty() {
        12.0
    } else {
        heights.iter().sum::<f64>() / heights.len() as f64
    };
    let y_tol = avg_height.max(6.0) * 0.5;

    // 按 Y 分行
    chars.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)
        .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)));
    let mut lines: Vec<Vec<(&Char, f64, f64)>> = vec![];
    for (c, x, y) in chars {
        if let Some(last) = lines.last_mut() {
            let last_y = last[0].2;
            if (y - last_y).abs() <= y_tol {
                last.push((c, x, y));
                continue;
            }
        }
        lines.push(vec![(c, x, y)]);
    }

    // 每行内按 X 排序 + 间距过滤 + 拼接
    lines.iter().map(|line| {
        let mut sorted = line.to_vec();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // 间距过滤：间距 < gap_threshold 的 char 视为重叠噪声，跳过
        // 但收集被跳过的 char，用于后续时间信息恢复
        let mut filtered: Vec<&Char> = vec![];
        let mut skipped: Vec<&Char> = vec![];
        for (c, x, _y) in sorted {
            if let Some(last_x) = filtered.last().map(|lc| lc.bbox.x0) {
                if x - last_x < gap_threshold {
                    // 间距过小，跳过此 char（视为重叠噪声）
                    skipped.push(c);
                    continue;
                }
            }
            filtered.push(c);
        }
        let mut text: String = filtered.iter().map(|c| c.text.as_str()).collect();

        // 时间信息恢复：从被跳过的 char 里找"数字+冒号"模式
        // 火车票 PDF 渲染顺序问题：小时 char（如"15"的"1"和"5"）可能与 day char
        // 在 X 上重叠被间距过滤跳过。如果 filtered text 的 "日" 后缺冒号，
        // 把被跳过的时间 char（数字+冒号）插到 "日" 后。
        let time_chars: String = skipped.iter()
            .filter(|c| c.text.chars().all(|ch| ch.is_ascii_digit() || ch == ':'))
            .map(|c| c.text.as_str())
            .collect();
        if !time_chars.is_empty() && text.contains('日') {
            let ri_pos = text.find('日').unwrap();
            let ri_end = ri_pos + '日'.len_utf8(); // "日" 是 3 字节 UTF-8 char
            let after_ri = &text[ri_end..];
            // "日" 后没有冒号 → 缺时间前缀，插回
            if !after_ri.contains(':') {
                text.insert_str(ri_end, &time_chars);
            }
        }
        text
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "pdfplumber")]
    use pdfplumber::TextDirection;

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

    // ── reconstruct_lines_from_chars tests ──

    #[cfg(feature = "pdfplumber")]
    #[test]
    fn test_reconstruct_lines_filters_noise_chars() {
        // 模拟火车票日期拆分：Word "2025年11月1"(chars: 2,0,2,5,年,1,1,月,1)
        // + Word "1"(噪声 char @x=124.7) + Word "4"(@x=133.5)
        // + Word "5日:22开"(chars: 5@136.7, 日@145.5, :@148.7, 2@160.7, 2@172.7, 开@184.7)
        // 间距 < avg_width*0.5 的噪声 char 被过滤，重建得 "2025年11月14日22开"
        let mk_char = |text: &str, x: f64, y: f64| pdfplumber::Char {
            text: text.to_string(),
            bbox: BBox::new(x, y, x + 12.0, y + 12.0),
            fontname: "SimSun".to_string(),
            size: 12.0,
            doctop: y,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 0,
            mcid: None,
            tag: None,
            render_mode: 0,
            text_object_index: 0,
        };
        let words = vec![
            pdfplumber::Word {
                text: "2025年11月1".to_string(),
                bbox: BBox::new(25.5, 134.6, 133.5, 146.6),
                doctop: 134.6,
                direction: TextDirection::Ltr,
                chars: vec![
                    mk_char("2", 25.5, 134.6), mk_char("0", 37.5, 134.6),
                    mk_char("2", 49.5, 134.6), mk_char("5", 61.5, 134.6),
                    mk_char("年", 73.5, 134.6), mk_char("1", 85.5, 134.6),
                    mk_char("1", 97.5, 134.6), mk_char("月", 109.5, 134.6),
                    mk_char("1", 121.5, 134.6),
                ],
                fontname: "SimSun".to_string(),
                size: 12.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
            pdfplumber::Word {
                text: "1".to_string(),
                bbox: BBox::new(124.7, 134.6, 136.7, 146.6),
                doctop: 134.6,
                direction: TextDirection::Ltr,
                chars: vec![mk_char("1", 124.7, 134.6)],
                fontname: "SimSun".to_string(),
                size: 12.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
            pdfplumber::Word {
                text: "4".to_string(),
                bbox: BBox::new(133.5, 134.6, 145.5, 146.6),
                doctop: 134.6,
                direction: TextDirection::Ltr,
                chars: vec![mk_char("4", 133.5, 134.6)],
                fontname: "SimSun".to_string(),
                size: 12.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
            pdfplumber::Word {
                text: "5日:22开".to_string(),
                bbox: BBox::new(136.7, 134.6, 196.7, 146.6),
                doctop: 134.6,
                direction: TextDirection::Ltr,
                chars: vec![
                    mk_char("5", 136.7, 134.6), mk_char("日", 145.5, 134.6),
                    mk_char(":", 148.7, 134.6), mk_char("2", 160.7, 134.6),
                    mk_char("2", 172.7, 134.6), mk_char("开", 184.7, 134.6),
                ],
                fontname: "SimSun".to_string(),
                size: 12.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
        ];
        let lines = reconstruct_lines_from_chars(&words);
        assert_eq!(lines.len(), 1, "should produce one line, got: {:?}", lines);
        // 间距过滤去掉噪声 "1"@124.7，但保留时间 "15:"（从被跳过的 char 恢复到 "日" 后）
        assert_eq!(lines[0], "2025年11月14日15:22开", "got: {}", lines[0]);
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
                fontname: String::new(),
                size: 0.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
            Word {
                text: "World".to_string(),
                bbox: BBox::new(55.0, 100.0, 95.0, 112.0),
                doctop: 100.0,
                direction: TextDirection::Ltr,
                chars: vec![],
                fontname: String::new(),
                size: 0.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
            Word {
                text: "Test".to_string(),
                bbox: BBox::new(100.0, 100.0, 130.0, 112.0),
                doctop: 100.0,
                direction: TextDirection::Ltr,
                chars: vec![],
                fontname: String::new(),
                size: 0.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
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
                fontname: String::new(),
                size: 0.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
            Word {
                text: "Line".to_string(),
                bbox: BBox::new(55.0, 100.0, 85.0, 112.0),
                doctop: 100.0,
                direction: TextDirection::Ltr,
                chars: vec![],
                fontname: String::new(),
                size: 0.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
            // Line 2 (Y = 150 — 50px below, well beyond 0.5 * avg_height)
            Word {
                text: "Second".to_string(),
                bbox: BBox::new(10.0, 150.0, 60.0, 162.0),
                doctop: 150.0,
                direction: TextDirection::Ltr,
                chars: vec![],
                fontname: String::new(),
                size: 0.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
            Word {
                text: "Line".to_string(),
                bbox: BBox::new(65.0, 150.0, 95.0, 162.0),
                doctop: 150.0,
                direction: TextDirection::Ltr,
                chars: vec![],
                fontname: String::new(),
                size: 0.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
            Word {
                text: "Too".to_string(),
                bbox: BBox::new(100.0, 150.0, 125.0, 162.0),
                doctop: 150.0,
                direction: TextDirection::Ltr,
                chars: vec![],
                fontname: String::new(),
                size: 0.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
            },
            // Line 3 (Y = 50 — 50px above, well beyond tolerance)
            Word {
                text: "Top".to_string(),
                bbox: BBox::new(10.0, 50.0, 40.0, 62.0),
                doctop: 50.0,
                direction: TextDirection::Ltr,
                chars: vec![],
                fontname: String::new(),
                size: 0.0,
                non_stroking_color: None,
                render_mode: 0,
                text_object_index: 0,
                font_flags: None,
                stem_v: None,
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
            fontname: String::new(),
            size: 0.0,
            non_stroking_color: None,
            render_mode: 0,
            text_object_index: 0,
            font_flags: None,
            stem_v: None,
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
