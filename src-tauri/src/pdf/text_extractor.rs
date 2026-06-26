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
    let pdf = Pdf::open_file(file_path, None).map_err(|e| format!("pdfplumber: {}", e))?;

    let mut results: Vec<OcrPageResult> = Vec::new();
    let mut total_words: usize = 0;

    for page_result in pdf.pages_iter() {
        let page = page_result.map_err(|e| format!("pdfplumber page: {}", e))?;
        let words = page.extract_words(&WordOptions::default());
        total_words += words.len();

        let lines = merge_words_into_lines(words);
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
}
