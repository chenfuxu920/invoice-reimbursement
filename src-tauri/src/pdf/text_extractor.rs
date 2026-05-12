use crate::ocr::OcrTextItem;
use serde::{Deserialize, Serialize};
use std::path::Path;

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
}
