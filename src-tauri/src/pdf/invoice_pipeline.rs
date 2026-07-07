use crate::models::invoice::{Invoice, InvoiceCategory, InvoiceSource, Itinerary};
use crate::ocr::OcrEngine;
use crate::parser::invoice_parser::parse_invoice_text;
use crate::parser::itinerary_parser::{parse_itinerary_text, parse_itinerary_with_coords_pages_and_fallback, cross_validate_amounts, enrich_itinerary_years, cross_validate_with_printed_total, extract_itinerary_printed_total, has_incomplete_entries, compute_incomplete_fields};
use crate::parser::dedup::deduplicate_invoices;
use crate::pdf::text_extractor::{self, classify_pdf_document_type, PdfDocumentType};
#[cfg(feature = "pdfplumber")]
use crate::parser::layout_extractor;
use std::path::{Path, PathBuf};

/// Check if a seller name looks garbled (failed extraction).
/// Used to trigger fallback even when seller_name is non-empty
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
    // Cipher field contamination — Chinese company names never contain < or >.
    // These characters come from invoice anti-forgery cipher fields (防伪码)
    // like "7*>+8-86923<505329>4-9*20-4" that get merged into the seller name
    // when pdfplumber's word merging mixes columns.
    if trimmed.contains('<') || trimmed.contains('>') {
        return true;
    }
    // Multi-line mixing — a legitimate seller name is a single line.
    // Newlines indicate words from different Y-rows were incorrectly merged.
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return true;
    }
    // Label artifact detection — if the name contains single-char label tokens
    // (购/买/售/销/方/密) as separate whitespace-delimited words, it's likely
    // column mixing from pdfplumber's word merging.
    let label_chars = "购买售销方密";
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words
        .iter()
        .any(|w| w.chars().count() == 1 && w.chars().all(|c| label_chars.contains(c)))
    {
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
    /// 行程单上印制的"合计金额"（精确值，用于匹配发票）
    /// None 表示未能从行程单中提取到合计金额（需回退容差匹配）
    pub printed_total: Option<f64>,
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

    // 单次 PDF 打开：列感知合并 + 原始 Word（供坐标提取器使用，无需二次打开）
    #[cfg(feature = "pdfplumber")]
    let (text_items, cached_words) = {
        match text_extractor::extract_pdf_column_aware(pdf_path) {
            Ok(extraction) => {
                let items: Vec<_> = extraction.pages.iter().flat_map(|p| p.texts.clone()).collect();
                if text_extractor::has_sufficient_text(&items, 20) {
                    eprintln!("  [pdfplumber] 列感知提取 {} 个文本项, {} 个原始Word", items.len(), extraction.raw_words.len());
                    (items, Some(extraction.raw_words))
                } else {
                    eprintln!("  [pdfplumber] 文本不足，回退到 parangi/OCR");
                    let fallback = extract_text(pdf_path, engine)?;
                    (fallback, None)
                }
            }
            Err(e) => {
                eprintln!("  [pdfplumber] 失败: {}，回退到 parangi/OCR", e);
                let fallback = extract_text(pdf_path, engine)?;
                (fallback, None)
            }
        }
    };
    #[cfg(not(feature = "pdfplumber"))]
    let text_items = extract_text(pdf_path, engine)?;

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
        Ok(invoice) if !invoice.seller_name.is_empty() && !is_likely_garbled_seller(&invoice.seller_name) && !invoice.invoice_number.is_empty() => Ok(invoice),
        Ok(mut invoice) => {
            // Seller 空/乱码或 invoice_number 缺失 — 先尝试 pdfplumber 原始 Word 坐标提取（比 OCR 快且准）
            #[cfg(feature = "pdfplumber")]
            {
                // 优先使用已缓存的 raw_words（单次 PDF 打开），无缓存时才重新提取
                let words_result = match &cached_words {
                    Some(w) => Ok(w.clone()),
                    None => text_extractor::extract_words_raw(pdf_path),
                };
                if let Ok(words) = words_result {
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

                    // 3. 如果 seller 和 invoice_number 都有效，直接返回（不走 parangi/OCR）
                    if !invoice.seller_name.is_empty()
                        && !is_likely_garbled_seller(&invoice.seller_name)
                        && !invoice.invoice_number.is_empty()
                    {
                        return Ok(invoice);
                    }
                }
            }
            // seller 或 invoice_number 仍缺失 — 尝试 parangi 纯文本交叉验证（比 OCR 快得多）
            let needs_seller = invoice.seller_name.is_empty() || is_likely_garbled_seller(&invoice.seller_name);
            let needs_invoice_number = invoice.invoice_number.is_empty();
            if needs_seller || needs_invoice_number {
                eprintln!("  [parangi] 交叉验证: seller_needed={}, invoice_number_needed={}", needs_seller, needs_invoice_number);
                if let Ok(parangi_items) = text_extractor::extract_text_from_pdf(pdf_path) {
                    if !text_extractor::is_garbled_items(&parangi_items, 0.3) {
                        let parangi_doc_type = classify_pdf_document_type(&parangi_items);
                        if parangi_doc_type == PdfDocumentType::Invoice {
                            if let Ok(parangi_invoice) =
                                parse_invoice_text(&parangi_items, source.clone())
                            {
                                // 合并：从 parangi 补全缺失字段，保留坐标提取的有效字段
                                if needs_seller
                                    && !parangi_invoice.seller_name.is_empty()
                                    && !is_likely_garbled_seller(&parangi_invoice.seller_name)
                                {
                                    invoice.seller_name = parangi_invoice.seller_name.clone();
                                    eprintln!("  [parangi] seller补全: {}", invoice.seller_name);
                                }
                                if needs_invoice_number
                                    && !parangi_invoice.invoice_number.is_empty()
                                {
                                    invoice.invoice_number = parangi_invoice.invoice_number.clone();
                                    eprintln!("  [parangi] invoice_number补全: {}", invoice.invoice_number);
                                }
                                // 日期补全：pdfplumber 多栏合并可能导致日期解析失败（默认 1970-01-01）
                                if invoice.date == chrono::NaiveDate::default()
                                    && parangi_invoice.date != chrono::NaiveDate::default()
                                {
                                    invoice.date = parangi_invoice.date;
                                    eprintln!("  [parangi] date补全: {}", invoice.date);
                                }
                                // 坐标提取的 amount 更可靠（多栏布局），但若为 0 则用 parangi 的
                                if invoice.amount <= 0.0 && parangi_invoice.amount > 0.0 {
                                    invoice.amount = parangi_invoice.amount;
                                }
                                // pdfplumber 多栏可能丢失备注区域 → hotel_detail 为 None/nights=1
                                // parangi 正确提取时覆盖
                                let pdf_nights = invoice.hotel_detail.as_ref().map(|d| d.nights).unwrap_or(1);
                                let parangi_nights = parangi_invoice.hotel_detail.as_ref().map(|d| d.nights).unwrap_or(1);
                                if parangi_nights > 1 && pdf_nights <= 1 {
                                    invoice.hotel_detail = parangi_invoice.hotel_detail.clone();
                                    eprintln!("  [parangi] hotel_detail补全: nights={}", parangi_nights);
                                }
                                // 如果 seller 现在有效，返回合并结果
                                if !invoice.seller_name.is_empty()
                                    && !is_likely_garbled_seller(&invoice.seller_name)
                                {
                                    eprintln!("  [parangi] 交叉验证完成");
                                    return Ok(invoice);
                                }
                            }
                        }
                    }
                }
            }
            // parangi 也未命中 — 尝试 zpdf（Form XObject 中的文字，带坐标）
            if let Ok(z_items) = text_extractor::extract_text_with_zpdf(pdf_path) {
                eprintln!("  [zpdf] 交叉验证: {} 个文本项", z_items.len());
                if let Ok(z_invoice) = parse_invoice_text(&z_items, source.clone()) {
                    if !z_invoice.seller_name.is_empty()
                        && !is_likely_garbled_seller(&z_invoice.seller_name)
                    {
                        eprintln!("  [zpdf] seller补全: {}", z_invoice.seller_name);
                        invoice.seller_name = z_invoice.seller_name;
                        if invoice.amount <= 0.0 && z_invoice.amount > 0.0 {
                            invoice.amount = z_invoice.amount;
                        }
                        if invoice.invoice_number.is_empty() && !z_invoice.invoice_number.is_empty() {
                            invoice.invoice_number = z_invoice.invoice_number;
                        }
                        return Ok(invoice);
                    }
                }
            }
            // OCR 回退
            // ponytail: OCR 不可用时返回文字提取结果（invoice_number/amount 通常已正确），
            // 而非硬报错。升级路径=安装 OCR 模型后自动走 OCR 补全 seller。
            if !engine.health().unwrap_or(false) {
                eprintln!("  [pipeline] OCR 不可用，返回文字提取结果");
                return Ok(invoice);
            }
            let ocr_pages = extract_ocr_text(pdf_path, engine)?;
            let ocr_items: Vec<_> = ocr_pages.iter().flat_map(|p| p.texts.clone()).collect();
            let ocr_doc_type = classify_pdf_document_type(&ocr_items);
            if ocr_doc_type == PdfDocumentType::Itinerary || ocr_doc_type == PdfDocumentType::Bill {
                return Err(format!("非发票类型（OCR回退）: {:?}", ocr_doc_type));
            }
            check_and_parse(ocr_items, source)
        }
        Err(_) => {
            // 解析失败 — 尝试 zpdf（Form XObject 中的文字）
            match text_extractor::extract_text_with_zpdf(pdf_path) {
                Ok(z_items) => {
                    eprintln!("  [zpdf] 提取到 {} 个文本项（parse失败回退）", z_items.len());
                    let z_doc_type = classify_pdf_document_type(&z_items);
                    if z_doc_type != PdfDocumentType::Itinerary && z_doc_type != PdfDocumentType::Bill {
                        if let Ok(invoice) = check_and_parse(z_items, source.clone()) {
                            return Ok(invoice);
                        }
                    }
                }
                Err(e) => eprintln!("  [zpdf] 失败: {}", e),
            }
            // OCR 回退
            if !engine.health().unwrap_or(false) {
                return Err("文字提取解析失败，且 OCR 模型未安装".to_string());
            }
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
            // ponytail: OCR 不可用时返回不足的文本，让后续解析尝试（可能部分成功）
            if !engine.health().unwrap_or(false) {
                return Ok(items);
            }
            let resp = engine.recognize_pdf(pdf_path)?;
            Ok(resp.pages.iter().flat_map(|p| p.texts.clone()).collect())
        }
        Err(e) => {
            eprintln!("  [parangi] 失败: {}，回退到 OCR", e);
            if !engine.health().unwrap_or(false) {
                return Err(format!("文字提取失败且 OCR 不可用: {}", e));
            }
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
    // 优先使用 extract_pdf_column_aware（含 pdfplumber 回退，保留页边界）
    // 多页行程单必须按页解析，否则 Y 坐标重叠导致表格解析失败
    #[cfg(feature = "pdfplumber")]
    {
        match text_extractor::extract_pdf_column_aware(pdf_path) {
            Ok(extraction) => {
                let flat_texts: Vec<_> = extraction.pages.iter().flat_map(|p| p.texts.clone()).collect();
                if text_extractor::has_sufficient_text(&flat_texts, 20) {
                    let doc_type = classify_pdf_document_type(&flat_texts);
                    if doc_type != PdfDocumentType::Itinerary && doc_type != PdfDocumentType::Invoice {
                        return Err(format!("非行程单类型: {:?}", doc_type));
                    }
                    eprintln!("  [pdfplumber] 列感知提取 {} 个文本项, {} 个原始Word ({} 页)",
                        flat_texts.len(), extraction.raw_words.len(), extraction.pages.len());

                    let has_coords = flat_texts.iter().any(|t| t.box_coords.is_some());

                    let itineraries = if has_coords {
                        eprintln!("  [pdfplumber] 行程单带坐标，按页 word 级坐标解析");
                        // 关键：用 word_pages（word 级未合并）按页解析，保留单元格坐标
                        let coord_result = parse_itinerary_with_coords_pages_and_fallback(
                            &extraction.word_pages, Some(&flat_texts));
                        if !coord_result.is_empty() && !has_incomplete_entries(&coord_result) {
                            coord_result
                        } else {
                            if !coord_result.is_empty() {
                                eprintln!("  [pdfplumber] 坐标解析有缺失字段，尝试纯文本解析");
                            } else {
                                eprintln!("  [pdfplumber] 按页坐标解析失败，尝试纯文本解析");
                            }
                            let mut text_result = parse_itinerary_text(&flat_texts);
                            if !text_result.is_empty() {
                                cross_validate_amounts(&mut text_result, &flat_texts);
                                let fb_text: String = flat_texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join("\n");
                                enrich_itinerary_years(&mut text_result, &fb_text);
                                if !has_incomplete_entries(&text_result) {
                                    text_result
                                } else {
                                    eprintln!("  [pdfplumber] 纯文本解析仍有缺失字段，回退到 OCR");
                                    let ocr_pages = extract_ocr_text(pdf_path, engine)?;
                                    parse_itinerary_with_coords_pages_and_fallback(&ocr_pages, Some(&flat_texts))
                                }
                            } else {
                                eprintln!("  [pdfplumber] 纯文本解析失败，回退到 OCR");
                                let ocr_pages = extract_ocr_text(pdf_path, engine)?;
                                parse_itinerary_with_coords_pages_and_fallback(&ocr_pages, Some(&flat_texts))
                            }
                        }
                    } else {
                        let ocr_pages = extract_ocr_text(pdf_path, engine)?;
                        parse_itinerary_with_coords_pages_and_fallback(&ocr_pages, Some(&flat_texts))
                    };

                    if itineraries.is_empty() {
                        return Err("行程单中未解析到行程明细".to_string());
                    }
                    return build_itinerary_doc(itineraries, &flat_texts, pdf_path);
                }
                eprintln!("  [pdfplumber] 文本不足，回退到 parangi/OCR");
            }
            Err(e) => {
                eprintln!("  [pdfplumber] 失败: {}，回退到 parangi/OCR", e);
            }
        }
    }

    // 回退路径：parangi/OCR（无坐标，纯文本）
    let texts = extract_text(pdf_path, engine)?;
    let doc_type = classify_pdf_document_type(&texts);
    if doc_type != PdfDocumentType::Itinerary && doc_type != PdfDocumentType::Invoice {
        return Err(format!("非行程单类型: {:?}", doc_type));
    }

    // 无坐标 — 走 OCR 获取坐标用于表格重建
    let ocr_pages = extract_ocr_text(pdf_path, engine)?;
    let mut itineraries = parse_itinerary_with_coords_pages_and_fallback(&ocr_pages, Some(&texts));
    if itineraries.is_empty() || has_incomplete_entries(&itineraries) {
        if !itineraries.is_empty() {
            eprintln!("  [parangi/OCR] 有缺失字段，尝试纯文本回退");
        }
        itineraries = parse_itinerary_text(&texts);
    }

    // 最后手段：所有回退都已尝试
    if itineraries.is_empty() {
        return Err("行程单中未解析到行程明细".to_string());
    }
    if has_incomplete_entries(&itineraries) {
        eprintln!("  [最终回退] 仍有缺失字段，但无更多回退路径");
    }
    build_itinerary_doc(itineraries, &texts, pdf_path)
}

/// 构建 ItineraryDoc，提取印制合计金额并交叉验证
fn build_itinerary_doc(
    mut itineraries: Vec<Itinerary>,
    texts: &[crate::ocr::OcrTextItem],
    pdf_path: &str,
) -> Result<ItineraryDoc, String> {
    compute_incomplete_fields(&mut itineraries);
    let printed_total = extract_itinerary_printed_total(texts);
    if let Some(pt) = printed_total {
        cross_validate_with_printed_total(&mut itineraries, pt);
        let file_name = Path::new(pdf_path).file_name()
            .unwrap_or_default().to_string_lossy().to_string();
        return Ok(ItineraryDoc { file_name, itineraries, total_amount: pt, printed_total: Some(pt) });
    }
    let total_amount: f64 = itineraries.iter().map(|i| i.amount).sum();
    let file_name = Path::new(pdf_path).file_name()
        .unwrap_or_default().to_string_lossy().to_string();
    Ok(ItineraryDoc { file_name, itineraries, total_amount, printed_total: None })
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
    pair_invoices_with_itineraries(&mut invoices, itinerary_docs, 2.0);

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

    pair_invoices_with_itineraries(&mut invoices, itinerary_docs, 2.0);

    // 批次内按发票号去重
    let duplicates = deduplicate_invoices(&mut invoices);

    ParseResult { invoices, errors, duplicates }
}

/// 将行程明细配对到对应的发票上（按总额匹配）
/// 匹配成功后自动将发票类别设为 CityTransport
pub fn pair_invoices_with_itineraries(
    invoices: &mut Vec<Invoice>,
    itinerary_docs: Vec<ItineraryDoc>,
    _tolerance: f64,  // 仅在没有合计金额时使用
) {
    for doc in itinerary_docs {
        // 如果有印制的合计金额，精确匹配（无需容差）
        if doc.printed_total.is_some() {
            let target = invoices.iter_mut().find(|inv| {
                inv.itineraries.is_empty()
                    && (inv.amount - doc.total_amount).abs() <= 0.01  // 浮点舍入容差
            });
            if let Some(inv) = target {
                inv.category = InvoiceCategory::CityTransport;
                inv.itineraries = doc.itineraries;
                inv.itinerary_file = Some(doc.file_name.clone());
                continue;
            }
        }
        // 没有合计金额时用容差匹配（回退逻辑）
        let target = invoices.iter_mut().find(|inv| {
            inv.itineraries.is_empty()
                && (inv.amount - doc.total_amount).abs() <= 2.00
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
