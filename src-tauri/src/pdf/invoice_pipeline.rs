use crate::models::invoice::{Invoice, InvoiceCategory, InvoiceSource, Itinerary};
use crate::ocr::OcrEngine;
use crate::parser::invoice_parser::parse_invoice_text;
use crate::parser::itinerary_parser::{parse_itinerary_text, parse_itinerary_with_coords_pages_and_fallback, enrich_itinerary_years, has_incomplete_entries, compute_incomplete_fields};
use crate::parser::dedup::deduplicate_invoices;
use crate::pdf::text_extractor::{self, classify_pdf_document_type, PdfDocumentType};
#[cfg(feature = "pdfplumber")]
use crate::parser::layout_extractor;
#[cfg(feature = "pdfplumber")]
use crate::parser::cell_extractor;
use std::path::{Path, PathBuf};



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

    // 单次 PDF 打开：列感知合并 + 原始 Word + 表格单元格（供坐标/单元格提取器使用，无需二次打开）
    #[cfg(feature = "pdfplumber")]
    let (text_items, cached_words, cached_tables) = {
        match text_extractor::extract_pdf_column_aware(pdf_path) {
            Ok(extraction) => {
                let items: Vec<_> = extraction.pages.iter().flat_map(|p| p.texts.clone()).collect();
                if text_extractor::has_sufficient_text(&items, 20) {
                    eprintln!("  [pdfplumber] 列感知提取 {} 个文本项, {} 个原始Word, {} 页表格", items.len(), extraction.raw_words.len(), extraction.tables.len());
                    (items, Some(extraction.raw_words), Some(extraction.tables))
                } else {
                    eprintln!("  [pdfplumber] 文本不足，回退到 OCR");
                    let fallback = extract_ocr_text_only(pdf_path, engine)?;
                    (fallback, None, None)
                }
            }
            Err(e) => {
                eprintln!("  [pdfplumber] 失败: {}，回退到 OCR", e);
                let fallback = extract_ocr_text_only(pdf_path, engine)?;
                (fallback, None, None)
            }
        }
    };
    #[cfg(not(feature = "pdfplumber"))]
    let text_items = extract_ocr_text_only(pdf_path, engine)?;



    // 先检查文档类型 — 行程单/结账单不应走发票解析
    let doc_type = classify_pdf_document_type(&text_items);
    if doc_type == PdfDocumentType::Itinerary || doc_type == PdfDocumentType::Bill {
        return Err(format!("非发票类型: {:?}", doc_type));
    }

    match check_and_parse(text_items, source.clone()) {
        Ok(mut invoice) => {
            // 单元格引导提取：用 find_tables 的表格结构补充/修正字段。
            // 注：不设提前返回守卫——即使 parse_invoice_text 成功提取了
            // seller/invoice_number，其金额可能误取商品详情行数值（Step 2.5 取最大裸小数），
            // 需用单元格从价税合计标签定位的正确金额覆盖。
            #[cfg(feature = "pdfplumber")]
            {
                if let Some(ref tables) = cached_tables {
                    let cell_fields = cell_extractor::extract_fields_from_tables(tables);
                    if invoice.seller_name.is_empty() {
                        if let Some(seller) = cell_fields.seller_name {
                            if seller.chars().count() >= 4 {
                                eprintln!("  [cell] seller补全: {}", seller);
                                invoice.seller_name = seller;
                            }
                        }
                    }
                    // 单元格提取的金额来自"价税合计"标签定位的值单元格，
                    // 比 parse_invoice_text 的全文正则（可能误取商品详情行金额）更可靠。
                    if let Some(amt) = cell_fields.amount {
                        if amt > 0.0 {
                            eprintln!("  [cell] amount: {} (cell)", amt);
                            invoice.amount = amt;
                        }
                    }
                    if invoice.item_name.is_empty() {
                        if let Some(item) = cell_fields.item_name {
                            invoice.item_name = item;
                        }
                    }
                    if invoice.remarks.is_empty() {
                        if let Some(remarks) = cell_fields.remarks {
                            invoice.remarks = remarks;
                        }
                    }
                    if invoice.hotel_detail.is_none() {
                        invoice.hotel_detail = cell_fields.hotel_detail;
                    }
                    // 单元格提取后如果 seller + invoice_number 都有效，直接返回
                    if !invoice.seller_name.is_empty()
                        && invoice.seller_name.chars().count() >= 4
                        && !invoice.invoice_number.is_empty()
                    {
                        return Ok(invoice);
                    }
                }
            }
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
                    if !seller.is_empty() && seller.chars().count() >= 4 {
                        invoice.seller_name = seller;
                    }

                    // 2. 坐标 amount 提取（紧凑 Y 带排除 items 表格的不含税金额）
                    if let Some(amt) = layout_extractor::extract_amount_by_coords(&words) {
                        if amt > 0.0 {
                            invoice.amount = amt;
                        }
                    }

                    // 3. 如果 seller 和 invoice_number 都有效，直接返回
                    if !invoice.seller_name.is_empty()
                        && invoice.seller_name.chars().count() >= 4
                        && !invoice.invoice_number.is_empty()
                    {
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

fn extract_ocr_text_only(pdf_path: &str, engine: &mut OcrEngine) -> Result<Vec<crate::ocr::OcrTextItem>, String> {
    if !engine.health().unwrap_or(false) {
        return Err("OCR 模型未安装".to_string());
    }
    let resp = engine.recognize_pdf(pdf_path)?;
    Ok(resp.pages.iter().flat_map(|p| p.texts.clone()).collect())
}

fn extract_ocr_text(pdf_path: &str, engine: &mut OcrEngine) -> Result<Vec<crate::ocr::OcrPageResult>, String> {
    let resp = engine.recognize_pdf(pdf_path)?;
    Ok(resp.pages)
}

/// 带坐标的文字提取：优先使用 pdfplumber（feature-gated），回退到 OCR
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
            eprintln!("  [pdfplumber] 不可用或无文本，回退到 OCR");
            extract_ocr_text_only(pdf_path, engine)
        }
    }
}

#[cfg(not(feature = "pdfplumber"))]
pub fn extract_text_with_coords_or_fallback(
    pdf_path: &str,
    engine: &mut OcrEngine,
) -> Result<Vec<crate::ocr::OcrTextItem>, String> {
    extract_ocr_text_only(pdf_path, engine)
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
                eprintln!("  [pdfplumber] 文本不足，回退到 OCR");
            }
            Err(e) => {
                eprintln!("  [pdfplumber] 失败: {}，回退到 OCR", e);
            }
        }
    }

    // 回退路径：OCR（无 pdfplumber 时）
    let ocr_pages = extract_ocr_text(pdf_path, engine)?;
    let ocr_items: Vec<_> = ocr_pages.iter().flat_map(|p| p.texts.clone()).collect();
    let doc_type = classify_pdf_document_type(&ocr_items);
    if doc_type != PdfDocumentType::Itinerary && doc_type != PdfDocumentType::Invoice {
        return Err(format!("非行程单类型: {:?}", doc_type));
    }

    let mut itineraries = parse_itinerary_with_coords_pages_and_fallback(&ocr_pages, Some(&ocr_items));

    if itineraries.is_empty() {
        eprintln!("  [OCR] 坐标解析无结果，尝试纯文本回退");
        itineraries = parse_itinerary_text(&ocr_items);
    }

    if itineraries.is_empty() {
        return Err("行程单中未解析到行程明细".to_string());
    }
    if has_incomplete_entries(&itineraries) {
        eprintln!("  [警告] 部分行程有时间字段不完整（OCR 乱码），已保留其余完整条目");
    }
    build_itinerary_doc(itineraries, &ocr_items, pdf_path)
}

/// 构建 ItineraryDoc
fn build_itinerary_doc(
    mut itineraries: Vec<Itinerary>,
    _texts: &[crate::ocr::OcrTextItem],
    pdf_path: &str,
) -> Result<ItineraryDoc, String> {
    compute_incomplete_fields(&mut itineraries);
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
