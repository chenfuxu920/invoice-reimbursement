use crate::ocr::engine::OcrEngine;
use crate::ocr::OcrTextItem;
use crate::pdf::image_embedder::render_pdf_to_rgb_images;
use serde::{Deserialize, Serialize};

#[cfg(feature = "pdfplumber")]
use crate::pdf::text_extractor::extract_raw_words_debug;
#[cfg(feature = "pdfplumber")]
use pdfplumber::{Pdf, TableSettings, WordOptions};

/// 调试界面单个文字项：坐标已统一到渲染图片像素空间（左上角 + 宽高）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugTextItem {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub confidence: f64,
}

/// 调试界面线条项：坐标已统一到渲染图片像素空间
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugLine {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub line_width: f64,
}

/// 调试界面矩形项：坐标已统一到渲染图片像素空间（左上角 + 宽高）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub line_width: f64,
    pub fill: bool,
}

/// 调试界面单元格项：find_tables 识别的单元格（坐标 + 文本内容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugCell {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub text: String,
}

/// 调试界面单页结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugPage {
    /// "data:image/png;base64,..."
    pub image: String,
    pub width: u32,
    pub height: u32,
    pub pdfplumber: Vec<DebugTextItem>,
    pub ocr: Vec<DebugTextItem>,
    pub lines: Vec<DebugLine>,
    pub rects: Vec<DebugRect>,
    pub cells: Vec<DebugCell>,
}

/// 调试日志：三个引擎各自的诊断日志行
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DebugLogs {
    pub pdfplumber: Vec<String>,
    pub ocr: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugTextResult {
    pub pages: Vec<DebugPage>,
    pub logs: DebugLogs,
}

/// 提取 PDF 文字并统一坐标到渲染图片像素空间。
/// `ocr_engine` 为 None 时 OCR 数组为空（不报错）。
pub fn debug_extract_texts(
    pdf_path: &str,
    dpi: u32,
    ocr_engine: Option<&mut OcrEngine>,
) -> Result<DebugTextResult, String> {
    let mut logs_pdfplumber: Vec<String> = Vec::new();
    let mut logs_ocr: Vec<String> = Vec::new();

    // 1. 渲染各页为图片 → base64
    let render_start = std::time::Instant::now();
    let images = render_pdf_to_rgb_images(pdf_path, dpi)?;
    let page_count = images.len();
    let render_log = format!("[渲染] DPI={dpi} 页数={page_count} 耗时={:.0}ms", render_start.elapsed().as_millis());
    logs_pdfplumber.push(render_log.clone());
    logs_ocr.push(render_log);

    // 2. pdfplumber 原始 word（PDF 点坐标），按页分组
    let pp_start = std::time::Instant::now();
    let pdfplumber_by_page = match extract_pdfplumber_by_page(pdf_path, page_count) {
        Ok(pages) => {
            let pp_total: usize = pages.iter().map(|v| v.len()).sum();
            logs_pdfplumber.push(format!("[提取] pdfplumber words={pp_total} 耗时={:.0}ms", pp_start.elapsed().as_millis()));
            for (i, words) in pages.iter().enumerate() {
                if !words.is_empty() {
                    logs_pdfplumber.push(format!("[页{i}] {} words", words.len()));
                }
            }
            pages
        }
        Err(e) => {
            logs_pdfplumber.push(format!("[提取] pdfplumber 失败: {e} → 降级为空 耗时={:.0}ms", pp_start.elapsed().as_millis()));
            vec![Vec::new(); page_count]
        }
    };

    // 2b. pdfplumber 线条和矩形（表格线条/单元格）
    let (lines_by_page, rects_by_page) = extract_pdfplumber_shapes_by_page(pdf_path, page_count);
    let lines_total: usize = lines_by_page.iter().map(|v| v.len()).sum();
    let rects_total: usize = rects_by_page.iter().map(|v| v.len()).sum();
    logs_pdfplumber.push(format!("[图形] lines={lines_total} rects={rects_total}"));

    // 2c. pdfplumber 表格单元格（find_tables 识别结果）
    let cells_by_page = extract_pdfplumber_tables_by_page(pdf_path, page_count);
    let cells_total: usize = cells_by_page.iter().map(|v| v.len()).sum();
    logs_pdfplumber.push(format!("[表格] cells={cells_total}"));

    // 3. OCR（200DPI 像素坐标），按页分组
    let ocr_by_page = if let Some(engine) = ocr_engine {
        let ocr_start = std::time::Instant::now();
        match extract_ocr_by_page(engine, pdf_path, page_count) {
            Ok(pages) => {
                let total: usize = pages.iter().map(|v| v.len()).sum();
                logs_ocr.push(format!("[提取] OCR items={total} 耗时={:.0}ms", ocr_start.elapsed().as_millis()));
                for (i, items) in pages.iter().enumerate() {
                    if !items.is_empty() {
                        logs_ocr.push(format!("[页{i}] {} items", items.len()));
                    }
                }
                pages
            }
            Err(e) => {
                logs_ocr.push(format!("[提取] OCR 失败: {e} → 降级为空"));
                vec![Vec::new(); page_count]
            }
        }
    } else {
        logs_ocr.push("[提取] OCR 引擎未加载（模型未下载）→ 跳过".to_string());
        vec![Vec::new(); page_count]
    };

    // 5. 组装：统一坐标到各页图片像素空间
    let scale_pt = dpi as f64 / 72.0;
    let scale_ocr = dpi as f64 / 200.0;

    let mut pages = Vec::with_capacity(page_count);
    for (i, img) in images.iter().enumerate() {
        let (w, h) = (img.width(), img.height());
        let image = rgb_to_png_data_uri(img)?;

        let pdfplumber: Vec<DebugTextItem> = pdfplumber_by_page
            .get(i)
            .map(|words| {
                words
                    .iter()
                    .map(|(text, x0, top, x1, bottom, _pn)| DebugTextItem {
                        text: text.clone(),
                        x: x0 * scale_pt,
                        y: top * scale_pt,
                        w: (x1 - x0) * scale_pt,
                        h: (bottom - top) * scale_pt,
                        confidence: 1.0,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let ocr: Vec<DebugTextItem> = ocr_by_page
            .get(i)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|it| parse_box_to_debug(it, scale_ocr))
                    .collect()
            })
            .unwrap_or_default();

        let lines: Vec<DebugLine> = lines_by_page
            .get(i)
            .map(|ls| {
                ls.iter()
                    .map(|(x0, top, x1, bottom, lw)| DebugLine {
                        x0: x0 * scale_pt,
                        y0: top * scale_pt,
                        x1: x1 * scale_pt,
                        y1: bottom * scale_pt,
                        line_width: lw * scale_pt,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let rects: Vec<DebugRect> = rects_by_page
            .get(i)
            .map(|rs| {
                rs.iter()
                    .map(|(x0, top, x1, bottom, lw, fill)| DebugRect {
                        x: x0 * scale_pt,
                        y: top * scale_pt,
                        w: (x1 - x0) * scale_pt,
                        h: (bottom - top) * scale_pt,
                        line_width: lw * scale_pt,
                        fill: *fill,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let cells: Vec<DebugCell> = cells_by_page
            .get(i)
            .map(|cs| {
                cs.iter()
                    .map(|(x0, top, x1, bottom, text)| DebugCell {
                        x: x0 * scale_pt,
                        y: top * scale_pt,
                        w: (x1 - x0) * scale_pt,
                        h: (bottom - top) * scale_pt,
                        text: text.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        pages.push(DebugPage {
            image,
            width: w,
            height: h,
            pdfplumber,
            ocr,
            lines,
            rects,
            cells,
        });
    }

    Ok(DebugTextResult {
        pages,
        logs: DebugLogs {
            pdfplumber: logs_pdfplumber,
            ocr: logs_ocr,
        },
    })
}

/// pdfplumber 原始 word 按页分组（page_number 从 1 开始）
#[cfg(feature = "pdfplumber")]
fn extract_pdfplumber_by_page(
    pdf_path: &str,
    page_count: usize,
) -> Result<Vec<Vec<(String, f64, f64, f64, f64, u32)>>, String> {
    let words = extract_raw_words_debug(pdf_path)?;
    let mut by_page: Vec<Vec<_>> = vec![Vec::new(); page_count];
    for w in words {
        let idx = (w.5 as usize).saturating_sub(1);
        if idx < page_count {
            by_page[idx].push(w);
        }
    }
    Ok(by_page)
}

#[cfg(not(feature = "pdfplumber"))]
fn extract_pdfplumber_by_page(
    _pdf_path: &str,
    page_count: usize,
) -> Result<Vec<Vec<(String, f64, f64, f64, f64, u32)>>, String> {
    Ok(vec![Vec::new(); page_count])
}

/// pdfplumber 线条和矩形按页分组（PDF 点坐标）
#[cfg(feature = "pdfplumber")]
fn extract_pdfplumber_shapes_by_page(
    pdf_path: &str,
    page_count: usize,
) -> (Vec<Vec<(f64, f64, f64, f64, f64)>>, Vec<Vec<(f64, f64, f64, f64, f64, bool)>>) {
    // ponytail: 调试工具，PDF 打开两次可接受（words 和 shapes 各一次）
    let result = std::panic::catch_unwind(|| {
        let pdf = match Pdf::open_file(pdf_path, None) {
            Ok(p) => p,
            Err(_) => return (vec![Vec::new(); page_count], vec![Vec::new(); page_count]),
        };
        let mut lines_by_page: Vec<Vec<(f64, f64, f64, f64, f64)>> = vec![Vec::new(); page_count];
        let mut rects_by_page: Vec<Vec<(f64, f64, f64, f64, f64, bool)>> = vec![Vec::new(); page_count];
        for page_result in pdf.pages_iter() {
            let page = match page_result {
                Ok(p) => p,
                Err(_) => continue,
            };
            // page_number() 在 pdfplumber fork 中已是 0-based
            let idx = page.page_number() as usize;
            if idx >= page_count {
                continue;
            }
            for line in page.lines() {
                lines_by_page[idx].push((line.x0, line.top, line.x1, line.bottom, line.line_width));
            }
            for rect in page.rects() {
                rects_by_page[idx].push((rect.x0, rect.top, rect.x1, rect.bottom, rect.line_width, rect.fill));
            }
        }
        (lines_by_page, rects_by_page)
    });
    match result {
        Ok((lines, rects)) => (lines, rects),
        Err(_) => (vec![Vec::new(); page_count], vec![Vec::new(); page_count]),
    }
}

#[cfg(not(feature = "pdfplumber"))]
fn extract_pdfplumber_shapes_by_page(
    _pdf_path: &str,
    page_count: usize,
) -> (Vec<Vec<(f64, f64, f64, f64, f64)>>, Vec<Vec<(f64, f64, f64, f64, f64, bool)>>) {
    (vec![Vec::new(); page_count], vec![Vec::new(); page_count])
}

/// pdfplumber find_tables 单元格按页分组（PDF 点坐标 + 单元格文本）
#[cfg(feature = "pdfplumber")]
fn extract_pdfplumber_tables_by_page(
    pdf_path: &str,
    page_count: usize,
) -> Vec<Vec<(f64, f64, f64, f64, String)>> {
    // ponytail: 调试工具，PDF 多次打开可接受（words/shapes/tables 各一次）
    let result = std::panic::catch_unwind(|| {
        let pdf = match Pdf::open_file(pdf_path, None) {
            Ok(p) => p,
            Err(_) => return vec![Vec::new(); page_count],
        };
        let mut cells_by_page: Vec<Vec<(f64, f64, f64, f64, String)>> = vec![Vec::new(); page_count];
        for page_result in pdf.pages_iter() {
            let page = match page_result {
                Ok(p) => p,
                Err(_) => continue,
            };
            let idx = page.page_number() as usize;
            if idx >= page_count {
                continue;
            }
            let tables = page.find_tables(&TableSettings::default());
            let total_cells: usize = tables.iter().map(|t| t.rows.iter().map(|r| r.len()).sum::<usize>()).sum();
            let page_area = page.width() * page.height();
            let table_area: f64 = tables.iter().map(|t| t.bbox.width() * t.bbox.height()).sum();

            // ponytail: lattice 只检测到 QR code 小框（<5% 页面积），回退到 word 级伪单元格
            let trivial = total_cells <= 2 && table_area < page_area * 0.05;

            if !trivial {
                for table in &tables {
                    for row in &table.rows {
                        for cell in row {
                            cells_by_page[idx].push((
                                cell.bbox.x0,
                                cell.bbox.top,
                                cell.bbox.x1,
                                cell.bbox.bottom,
                                cell.text.clone().unwrap_or_default(),
                            ));
                        }
                    }
                }
            } else {
                // 回退：每个 pdfplumber word 作为一个单元格
                let words = page.extract_words(&WordOptions::default());
                for w in &words {
                    cells_by_page[idx].push((
                        w.bbox.x0,
                        w.bbox.top,
                        w.bbox.x1,
                        w.bbox.bottom,
                        w.text.clone(),
                    ));
                }
            }
        }
        cells_by_page
    });
    result.unwrap_or_else(|_| vec![Vec::new(); page_count])
}

#[cfg(not(feature = "pdfplumber"))]
fn extract_pdfplumber_tables_by_page(
    _pdf_path: &str,
    page_count: usize,
) -> Vec<Vec<(f64, f64, f64, f64, String)>> {
    vec![Vec::new(); page_count]
}

/// OCR 按页分组（recognize_pdf 返回 OcrPdfResponse { pages: [{page, texts}] }）
fn extract_ocr_by_page(
    engine: &mut OcrEngine,
    pdf_path: &str,
    page_count: usize,
) -> Result<Vec<Vec<OcrTextItem>>, String> {
    let resp = engine.recognize_pdf(pdf_path);
    match resp {
        Ok(r) => {
            let mut by_page = vec![Vec::new(); page_count];
            for p in r.pages {
                let idx = (p.page as usize).saturating_sub(1);
                if idx < page_count {
                    by_page[idx] = p.texts;
                }
            }
            Ok(by_page)
        }
        Err(_) => Ok(vec![Vec::new(); page_count]),
    }
}

/// 从 OcrTextItem.box_coords（{points:[{x,y}*4], box_score}）解析为 DebugTextItem
fn parse_box_to_debug(item: &OcrTextItem, scale: f64) -> Option<DebugTextItem> {
    let coords = item.box_coords.as_ref()?;
    let points = coords.get("points")?.as_array()?;
    if points.len() < 4 {
        return None;
    }
    let mut xs = [0.0f64; 4];
    let mut ys = [0.0f64; 4];
    for (i, p) in points.iter().take(4).enumerate() {
        xs[i] = p.get("x")?.as_f64()?;
        ys[i] = p.get("y")?.as_f64()?;
    }
    let x0 = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let x1 = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let y0 = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let y1 = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Some(DebugTextItem {
        text: item.text.clone(),
        x: x0 * scale,
        y: y0 * scale,
        w: (x1 - x0) * scale,
        h: (y1 - y0) * scale,
        confidence: item.confidence,
    })
}

/// RgbImage → PNG → base64 data URI
fn rgb_to_png_data_uri(img: &image::RgbImage) -> Result<String, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("PNG 编码失败: {:?}", e))?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    Ok(format!("data:image/png;base64,{b64}"))
}
