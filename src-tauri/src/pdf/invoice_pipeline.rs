use crate::models::invoice::{Invoice, InvoiceCategory, InvoiceSource, Itinerary};
use crate::ocr::OcrEngine;
use crate::parser::invoice_parser::parse_invoice_text;
use crate::parser::itinerary_parser::{parse_itinerary_text, parse_itinerary_with_coords_pages_and_fallback};
use crate::pdf::text_extractor::{self, classify_pdf_document_type, PdfDocumentType};
use std::path::{Path, PathBuf};

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
}

/// 解析单个发票 PDF：先尝试文字提取，失败或缺销售方信息则 OCR（多页）
pub fn parse_invoice_from_pdf(pdf_path: &str, engine: &mut OcrEngine) -> Result<Invoice, String> {
    let source = InvoiceSource::Pdf(pdf_path.to_string());
    let text_items = extract_text(pdf_path, engine)?;
    match check_and_parse(text_items, source.clone()) {
        Ok(invoice) if !invoice.seller_name.is_empty() => Ok(invoice),
        Ok(_) | Err(_) => {
            // parangi text may have scrambled multi-column layout; fall back to OCR
            let ocr_pages = extract_ocr_text(pdf_path, engine)?;
            let ocr_items: Vec<_> = ocr_pages.iter().flat_map(|p| p.texts.clone()).collect();
            check_and_parse(ocr_items, source)
        }
    }
}

fn extract_text(pdf_path: &str, engine: &mut OcrEngine) -> Result<Vec<crate::ocr::OcrTextItem>, String> {
    match text_extractor::extract_text_from_pdf(pdf_path) {
        Ok(items) if text_extractor::has_sufficient_text(&items, 20) => Ok(items),
        _ => {
            let resp = engine.recognize_pdf(pdf_path)?;
            Ok(resp.pages.iter().flat_map(|p| p.texts.clone()).collect())
        }
    }
}

fn extract_ocr_text(pdf_path: &str, engine: &mut OcrEngine) -> Result<Vec<crate::ocr::OcrPageResult>, String> {
    let resp = engine.recognize_pdf(pdf_path)?;
    Ok(resp.pages)
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
/// 行程单优先走 OCR 路径以保留坐标，利用坐标还原表格行列结构；
/// OCR 失败时回退到纯文本解析。
pub fn parse_itinerary_from_pdf(pdf_path: &str, engine: &mut OcrEngine) -> Result<ItineraryDoc, String> {
    let texts = extract_text(pdf_path, engine)?;
    let doc_type = classify_pdf_document_type(&texts);
    if doc_type != PdfDocumentType::Itinerary && doc_type != PdfDocumentType::Invoice {
        return Err(format!("非行程单类型: {:?}", doc_type));
    }

    let ocr_pages = extract_ocr_text(pdf_path, engine)?;
    let ocr_result = parse_itinerary_with_coords_pages_and_fallback(&ocr_pages, Some(&texts));
    let itineraries = if !ocr_result.is_empty() {
        ocr_result
    } else {
        parse_itinerary_text(&texts)
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
        Err(_) => return ParseResult { invoices, errors },
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

    ParseResult { invoices, errors }
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

    ParseResult { invoices, errors }
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
            });
        }
    }
}
