use crate::models::invoice::{Invoice, InvoiceCategory, InvoiceSource, Itinerary};
use crate::ocr::OcrEngine;
use crate::parser::invoice_parser::parse_invoice_text;
use crate::parser::itinerary_parser::{parse_itinerary_text, parse_itinerary_with_coords, parse_itinerary_with_coords_pages_and_fallback};
use crate::parser::dedup::deduplicate_invoices;
use crate::pdf::text_extractor::{self, classify_pdf_document_type, PdfDocumentType};
#[cfg(feature = "pdfplumber")]
use crate::parser::layout_extractor;
use std::path::{Path, PathBuf};

/// Check if a seller name looks garbled (failed extraction).
/// Used to trigger OCR fallback even when seller_name is non-empty
/// but clearly wrong (e.g., contains the label itself or is too short).
fn is_likely_garbled_seller(name: &str) -> bool {
    let trimmed = name.trim();
    // Too short to be a real company name (Chinese company names are typically 4+ chars)
    if trimmed.chars().count() < 3 {
        return true;
    }
    // Contains the label itself — extraction got "名称：" but not the actual name
    if trimmed.contains("名称：") || trimmed.contains("名称:") {
        return true;
    }
    // Contains only punctuation, whitespace, or common label characters
    if trimmed.chars().all(|c| {
        c.is_whitespace() || "名称：:，,。.、；;（）()".contains(c)
    }) {
        return true;
    }
    false
}

/// 从行程单解析出的行程明细集合
#[derive(Debug, Clone, serde::Serialize)]
pub struct ItineraryDoc {
    pub file_name: String,
    pub itineraries: Vec<Itinerary>,
    pub total_amount: f64,
}

/// 解析目录结果
#[derive(Debug, serde::Serialize)]
pub struct ParseResult {
    pub invoices: Vec<Invoice>,
    pub errors: Vec<(String, String)>,
    /// 批次内去重命中的重复发票号列表
    pub duplicates: Vec<String>,
}

/// 解析单个发票 PDF：先尝试文字提取，失败或缺销售方信息则 OCR（多页）
/// 如果文档分类为行程单/结账单，直接返回错误（不应当发票处理）
pub fn parse_invoice_from_pdf(pdf_path: &str, engine: &mut OcrEngine) -> Result<Invoice, String> {
    let source = InvoiceSource::Pdf(pdf_path.to_string());
    let text_items = extract_text_with_coords_or_fallback(pdf_path, engine)?;

    // 乱码检测：pdfplumber 对 CID 字体 PDF 可能输出乱码（如铁路电子客票），
    // 检测到乱码时回退到 parangi（有 UCS2 CMap 回退，能正确提取 CID 字体）
    let text_items = if text_extractor::is_garbled_items(&text_items, 0.3) {
        eprintln!("  [pdfplumber] 检测到CID乱码，回退到parangi");
        match text_extractor::extract_text_from_pdf(pdf_path) {
            Ok(parangi_items) if !text_extractor::is_garbled_items(&parangi_items, 0.3) => {
                parangi_items
            }
            _ => text_items, // parangi 也乱码或失败，保留原结果走后续解析/OCR
        }
    } else {
        text_items
    };

    // 先检查文档类型 — 行程单/结账单不应走发票解析
    let doc_type = classify_pdf_document_type(&text_items);
    if doc_type == PdfDocumentType::Itinerary || doc_type == PdfDocumentType::Bill {
        return Err(format!("非发票类型: {:?}", doc_type));
    }

    match check_and_parse(text_items, source.clone()) {
        Ok(invoice) if !invoice.seller_name.is_empty() && !is_likely_garbled_seller(&invoice.seller_name) => Ok(invoice),
        Ok(mut invoice) => {
            // Seller 空/乱码或 Amount 错误 — 先尝试 pdfplumber 原始 Word 坐标提取（比 OCR 快且准）
            #[cfg(feature = "pdfplumber")]
            {
                if let Ok(words) = text_extractor::extract_words_raw(pdf_path) {
                    // 1. 坐标 seller 提取
                    let seller = layout_extractor::extract_seller_by_raw_coords(&words);
                    if !seller.is_empty() && !is_likely_garbled_seller(&seller) {
                        invoice.seller_name = seller;
                    }

                    // 2. 坐标 amount 提取（紧凑 Y 带排除 items 表格的不含税金额）
                    if let Some(amt) = layout_extractor::extract_amount_by_coords(&words) {
                        if amt > 0.0 {
                            invoice.amount = amt;
                        }
                    }

                    // 3. 如果 seller 现在有效，直接返回（不走 OCR）
                    if !invoice.seller_name.is_empty()
                        && !is_likely_garbled_seller(&invoice.seller_name)
                    {
                        return Ok(invoice);
                    }
                }
            }
            // 坐标提取未命中 — OCR 回退
            let ocr_pages = extract_ocr_text(pdf_path, engine)?;
            let ocr_items: Vec<_> = ocr_pages.iter().flat_map(|p| p.texts.clone()).collect();
            let ocr_doc_type = classify_pdf_document_type(&ocr_items);
            if ocr_doc_type == PdfDocumentType::Itinerary || ocr_doc_type == PdfDocumentType::Bill {
                return Err(format!("非发票类型（OCR回退）: {:?}", ocr_doc_type));
            }
            check_and_parse(ocr_items, source)
        }
        Err(_) => {
            // 解析失败 — OCR 回退
            let ocr_pages = extract_ocr_text(pdf_path, engine)?;
            let ocr_items: Vec<_> = ocr_pages.iter().flat_map(|p| p.texts.clone()).collect();
            let ocr_doc_type = classify_pdf_document_type(&ocr_items);
            if ocr_doc_type == PdfDocumentType::Itinerary || ocr_doc_type == PdfDocumentType::Bill {
                return Err(format!("非发票类型（OCR回退）: {:?}", ocr_doc_type));
            }
            check_and_parse(ocr_items, source)
        }
    }
}

fn extract_text(pdf_path: &str, engine: &mut OcrEngine) -> Result<Vec<crate::ocr::OcrTextItem>, String> {
    match text_extractor::extract_text_from_pdf(pdf_path) {
        Ok(items) if text_extractor::has_sufficient_text(&items, 20) => {
            eprintln!("  [parangi] 提取到 {} 个文本项", items.len());
            Ok(items)
        }
        Ok(items) => {
            eprintln!("  [parangi] 文本不足 ({} 字符)，回退到 OCR", items.iter().map(|i| i.text.len()).sum::<usize>());
            let resp = engine.recognize_pdf(pdf_path)?;
            Ok(resp.pages.iter().flat_map(|p| p.texts.clone()).collect())
        }
        Err(e) => {
            eprintln!("  [parangi] 失败: {}，回退到 OCR", e);
            let resp = engine.recognize_pdf(pdf_path)?;
            Ok(resp.pages.iter().flat_map(|p| p.texts.clone()).collect())
        }
    }
}

fn extract_ocr_text(pdf_path: &str, engine: &mut OcrEngine) -> Result<Vec<crate::ocr::OcrPageResult>, String> {
    let resp = engine.recognize_pdf(pdf_path)?;
    Ok(resp.pages)
}

/// 带坐标的文字提取：优先使用 pdfplumber（feature-gated），回退到 parangi/OCR
#[cfg(feature = "pdfplumber")]
pub fn extract_text_with_coords_or_fallback(
    pdf_path: &str,
    engine: &mut OcrEngine,
) -> Result<Vec<crate::ocr::OcrTextItem>, String> {
    match text_extractor::extract_text_with_coords_flat(pdf_path) {
        Ok(items) if text_extractor::has_sufficient_text(&items, 20) => {
            eprintln!("  [pdfplumber] 提取到 {} 个带坐标文本项", items.len());
            Ok(items)
        }
        _ => {
            eprintln!("  [pdfplumber] 不可用或无文本，回退到 parangi/OCR");
            extract_text(pdf_path, engine)
        }
    }
}

#[cfg(not(feature = "pdfplumber"))]
pub fn extract_text_with_coords_or_fallback(
    pdf_path: &str,
    engine: &mut OcrEngine,
) -> Result<Vec<crate::ocr::OcrTextItem>, String> {
    extract_text(pdf_path, engine)
}

/// 解析单个发票图片：OCR 识别后分类检查
pub fn parse_invoice_from_image(image_path: &str, engine: &mut OcrEngine) -> Result<Invoice, String> {
    let source = InvoiceSource::Photo(image_path.to_string());
    let resp = engine.recognize_image(image_path)?;
    check_and_parse(resp.texts, source)
}

fn check_and_parse(
    text_items: Vec<crate::ocr::OcrTextItem>,
    source: InvoiceSource,
) -> Result<Invoice, String> {
    let doc_type = classify_pdf_document_type(&text_items);
    if doc_type != PdfDocumentType::Invoice {
        return Err(format!("非发票类型: {:?}", doc_type));
    }
    parse_invoice_text(&text_items, source)
}

/// 解析行程单 PDF，返回行程明细集合
/// 当 pdfplumber 可用时优先使用带坐标文本（跳过 OCR）；
/// 否则走 OCR 路径以保留坐标，利用坐标还原表格行列结构；
/// 均失败时回退到纯文本解析。
pub fn parse_itinerary_from_pdf(pdf_path: &str, engine: &mut OcrEngine) -> Result<ItineraryDoc, String> {
    let texts = extract_text_with_coords_or_fallback(pdf_path, engine)?;
    let doc_type = classify_pdf_document_type(&texts);
    if doc_type != PdfDocumentType::Itinerary && doc_type != PdfDocumentType::Invoice {
        return Err(format!("非行程单类型: {:?}", doc_type));
    }

    // Check if text items already have coordinates (from pdfplumber)
    let has_coords = texts.iter().any(|t| t.box_coords.is_some());

    let itineraries = if has_coords {
        // Text items have coordinates (from pdfplumber) — try coord-based parsing first
        eprintln!("  [pdfplumber] 行程单带坐标，尝试坐标解析");
        let coord_result = parse_itinerary_with_coords(&texts);
        if !coord_result.is_empty() {
            coord_result
        } else {
            // Coords didn't help — try text-only parsing
            let text_result = parse_itinerary_text(&texts);
            if !text_result.is_empty() {
                text_result
            } else {
                // Both failed — fall back to OCR for better table reconstruction
                eprintln!("  [pdfplumber] 坐标和文本解析均失败，回退到 OCR");
                let ocr_pages = extract_ocr_text(pdf_path, engine)?;
                let ocr_result = parse_itinerary_with_coords_pages_and_fallback(&ocr_pages, Some(&texts));
                if !ocr_result.is_empty() {
                    ocr_result
                } else {
                    parse_itinerary_text(&texts)
                }
            }
        }
    } else {
        // No coords (parangi or OCR fallback) — run OCR for coordinate-based table reconstruction
        let ocr_pages = extract_ocr_text(pdf_path, engine)?;
        let ocr_result = parse_itinerary_with_coords_pages_and_fallback(&ocr_pages, Some(&texts));
        if !ocr_result.is_empty() {
            ocr_result
        } else {
            parse_itinerary_text(&texts)
        }
    };

    if itineraries.is_empty() {
        return Err("行程单中未解析到行程明细".to_string());
    }
    let total_amount: f64 = itineraries.iter().map(|i| i.amount).sum();
    let file_name = Path::new(pdf_path).file_name()
        .unwrap_or_default().to_string_lossy().to_string();
    Ok(ItineraryDoc { file_name, itineraries, total_amount })
}

/// 批量解析目录下所有 PDF（发票+行程单），自动配对
/// 行程单解析结果会被配对到对应的 CityTransport 发票上（按总额匹配）
pub fn parse_all_from_dir(
    dir: &str,
    engine: &mut OcrEngine,
) -> ParseResult {
    let mut invoices = Vec::new();
    let mut errors = Vec::new();
    let mut itinerary_docs = Vec::new();

    let pdf_files: Vec<PathBuf> = match Path::new(dir).read_dir() {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "pdf"))
            .map(|e| e.path())
            .collect(),
        Err(_) => return ParseResult { invoices, errors, duplicates: Vec::new() },
    };

    for path in &pdf_files {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        // 先尝试以发票解析
        match parse_invoice_from_pdf(path.to_str().unwrap(), engine) {
            Ok(inv) => { invoices.push(inv); continue; }
            Err(_) => {}
        }
        // 发票解析失败，尝试以行程单解析
        match parse_itinerary_from_pdf(path.to_str().unwrap(), engine) {
            Ok(doc) => itinerary_docs.push(doc),
            Err(e) => errors.push((name, e)),
        }
    }

    // 配对：将行程单与 CityTransport 发票关联
    pair_invoices_with_itineraries(&mut invoices, itinerary_docs, 0.01);

    // 批次内按发票号去重
    let duplicates = deduplicate_invoices(&mut invoices);

    ParseResult { invoices, errors, duplicates }
}

/// 批量识别文件列表（发票+行程单），自动匹配行程单到发票
pub fn parse_all_from_files(
    files: &[String],
    engine: &mut OcrEngine,
) -> ParseResult {
    let mut invoices = Vec::new();
    let mut errors = Vec::new();
    let mut itinerary_docs = Vec::new();

    for path_str in files {
        let path = Path::new(path_str);
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        // 先尝试以发票解析
        match parse_invoice_from_pdf(path_str, engine) {
            Ok(inv) => { invoices.push(inv); continue; }
            Err(_) => {}
        }
        // 发票解析失败，尝试以行程单解析
        match parse_itinerary_from_pdf(path_str, engine) {
            Ok(doc) => itinerary_docs.push(doc),
            Err(e) => errors.push((name, e)),
        }
    }

    pair_invoices_with_itineraries(&mut invoices, itinerary_docs, 0.01);

    // 批次内按发票号去重
    let duplicates = deduplicate_invoices(&mut invoices);

    ParseResult { invoices, errors, duplicates }
}

/// 将行程明细配对到对应的发票上（按总额匹配）
/// 匹配成功后自动将发票类别设为 CityTransport
pub fn pair_invoices_with_itineraries(
    invoices: &mut Vec<Invoice>,
    itinerary_docs: Vec<ItineraryDoc>,
    tolerance: f64,
) {
    for doc in itinerary_docs {
        // 找一张金额匹配且尚未关联行程的发票（不限类别）
        let target = invoices.iter_mut().find(|inv| {
            inv.itineraries.is_empty()
                && (inv.amount - doc.total_amount).abs() <= tolerance
        });
        if let Some(inv) = target {
            inv.category = InvoiceCategory::CityTransport;
            inv.itineraries = doc.itineraries;
            inv.itinerary_file = Some(doc.file_name.clone());
        } else {
            // 没有匹配的发票，创建一张虚拟发票
            let id = uuid::Uuid::new_v4().to_string();
            invoices.push(Invoice {
                id,
                invoice_number: String::new(),
                amount: doc.total_amount,
                seller_name: "市内交通".to_string(),
                item_name: "市内交通".to_string(),
                date: chrono::NaiveDate::default(),
                travel_date: None,
                category: InvoiceCategory::CityTransport,
                source: InvoiceSource::Pdf(doc.file_name.clone()),
                itineraries: doc.itineraries,
                itinerary_file: Some(doc.file_name.clone()),
                remarks: String::new(),
                hotel_detail: None,
                departure_city: None,
                arrival_city: None,
                            toll_travel_time: None,
            });
        }
    }
}
