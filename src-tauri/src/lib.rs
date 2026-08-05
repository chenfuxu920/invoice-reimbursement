pub mod commands;
pub mod matching;
pub mod models;
pub mod ocr;
pub mod parser;
pub mod pdf;

use ocr::OcrEngine;
use parser::itinerary_parser::{parse_itinerary_text, parse_itinerary_with_coords, compute_incomplete_fields};
use parser::wechat_parser;
use parser::alipay_parser;
use models::invoice::{Invoice, Itinerary};
use models::payment::PaymentRecord;
use models::match_result::{MatchResult, ItineraryPaymentPair};
use crate::models::reimbursement_config::{self, ReimbursementConfig};
use matching::batch;
use matching::manual;
use crate::pdf::form_generator;
use crate::pdf::comparison_html_generator;
use crate::pdf::comparison_generator;
use crate::pdf::comparison_image_pdf_generator;
use crate::pdf::form_builder;
use crate::pdf::form_html_generator;
use crate::pdf::form_xlsx_generator;
use crate::pdf::comparison_xlsx_generator;
use crate::pdf::invoice_pipeline::{self, ParseResult};
use tokio::sync::Mutex as AsyncMutex;
use tauri::{Emitter, Manager};

// 应用状态
pub struct AppState {
    ocr_engine: AsyncMutex<OcrEngine>,
}

// OCR 健康检查命令
#[tauri::command]
async fn ocr_health(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let engine = state.ocr_engine.lock().await;
    engine.health()
}

// OCR 图片识别命令
#[tauri::command]
async fn ocr_recognize_image(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<serde_json::Value, String> {
    let mut engine = state.ocr_engine.lock().await;
    let result = engine.recognize_image(&file_path)?;
    Ok(serde_json::to_value(result).map_err(|e| e.to_string())?)
}

// OCR PDF 识别命令
#[tauri::command]
async fn ocr_recognize_pdf(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<serde_json::Value, String> {
    let mut engine = state.ocr_engine.lock().await;
    let result = engine.recognize_pdf(&file_path)?;
    Ok(serde_json::to_value(result).map_err(|e| e.to_string())?)
}

// 下载 OCR 模型（识别扫描件/图片发票，完成后热替换引擎并通知前端）
#[tauri::command]
async fn download_ocr_models(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let models_dir = ocr::model_downloader::download_models(&app).await?;
    let dir_str = models_dir.to_string_lossy().to_string();
    let engine = OcrEngine::new(&dir_str)
        .map_err(|e| format!("模型下载完成但初始化失败: {}", e))?;
    *state.ocr_engine.lock().await = engine;
    // 引擎已热替换，此时发出完成事件，前端健康检查可读到新引擎
    let _ = app.emit("ocr-download-complete", ());
    Ok(())
}

// 获取 OCR 模型下载配置
#[tauri::command]
async fn get_ocr_model_config(
    app: tauri::AppHandle,
) -> Result<ocr::OcrModelConfig, String> {
    Ok(ocr::model_downloader::load_config(&app))
}

// 设置 OCR 模型下载地址
#[tauri::command]
async fn set_ocr_model_config(
    app: tauri::AppHandle,
    model_base_url: String,
) -> Result<(), String> {
    let config = ocr::OcrModelConfig { model_base_url };
    ocr::model_downloader::save_config(&app, &config)
}

// 获取报销标准配置
#[tauri::command]
async fn get_reimbursement_config(app: tauri::AppHandle) -> Result<ReimbursementConfig, String> {
    Ok(reimbursement_config::load_config(&app))
}

// 获取当前生效的内置住宿标准（省份→城市层级，供设置页展示默认标准）
#[tauri::command]
async fn get_builtin_hotel_standards() -> Result<Vec<crate::models::reimbursement_config::ProvinceStandard>, String> {
    Ok(crate::models::hotel_standard::get_builtin_hotel_standards())
}

// 设置报销标准配置
#[tauri::command]
async fn set_reimbursement_config(app: tauri::AppHandle, config: ReimbursementConfig) -> Result<(), String> {
    let cfg = reimbursement_config::sanitize(config);
    reimbursement_config::save_config(&app, &cfg)?;
    reimbursement_config::apply_config(&cfg);
    Ok(())
}

// 发票识别与解析命令
#[tauri::command]
async fn recognize_invoice(
    state: tauri::State<'_, AppState>,
    file_path: String,
    file_type: String, // "image" | "pdf"
) -> Result<Invoice, String> {
    let mut engine = state.ocr_engine.lock().await;
    if file_type == "pdf" {
        invoice_pipeline::parse_invoice_from_pdf(&file_path, &mut engine, &Default::default())
    } else {
        invoice_pipeline::parse_invoice_from_image(&file_path, &mut engine)
    }
}

// 批量识别文件（发票+行程单），自动匹配行程单到发票
#[tauri::command]
async fn batch_recognize(
    state: tauri::State<'_, AppState>,
    file_paths: Vec<String>,
) -> Result<ParseResult, String> {
    let mut engine = state.ocr_engine.lock().await;
    Ok(invoice_pipeline::parse_all_from_files(&file_paths, &mut engine, &Default::default()))
}

// 行程单识别与解析命令
#[tauri::command]
async fn recognize_itinerary(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<Vec<Itinerary>, String> {
    let mut engine = state.ocr_engine.lock().await;

    let texts = invoice_pipeline::extract_text_with_coords_or_fallback(&file_path, &mut engine)?;

    // If text items have coordinates (from pdfplumber), use coord-based parsing
    let has_coords = texts.iter().any(|t| t.box_coords.is_some());
    let mut itineraries = if has_coords {
        let coord_result = parse_itinerary_with_coords(&texts);
        if !coord_result.is_empty() {
            coord_result
        } else {
            parse_itinerary_text(&texts)
        }
    } else {
        parse_itinerary_text(&texts)
    };

    compute_incomplete_fields(&mut itineraries);
    Ok(itineraries)
}

// 微信账单导入命令
#[tauri::command]
async fn import_wechat_bill(file_path: String) -> Result<Vec<PaymentRecord>, String> {
    wechat_parser::parse_wechat_bill(&file_path)
}

// 支付宝账单导入命令
#[tauri::command]
async fn import_alipay_bill(file_path: String) -> Result<Vec<PaymentRecord>, String> {
    alipay_parser::parse_alipay_bill(&file_path)
}

// 账单自动识别导入命令（微信/支付宝按内容自动判断）
#[tauri::command]
async fn import_bill(file_path: String) -> Result<Vec<PaymentRecord>, String> {
    parser::parse_bill_auto(&file_path)
}

// 自动批量匹配命令
#[tauri::command]
async fn auto_match(
    invoices: Vec<Invoice>,
    payments: Vec<PaymentRecord>,
    tolerance: f64,
) -> Result<serde_json::Value, String> {
    let result = batch::batch_match(&invoices, &payments, tolerance);
    Ok(serde_json::to_value(result).map_err(|e| e.to_string())?)
}

// 手动匹配命令
#[tauri::command]
async fn manual_match(
    invoice: Invoice,
    payments: Vec<PaymentRecord>,
    itinerary_payment_pairs: Vec<ItineraryPaymentPair>,
) -> Result<MatchResult, String> {
    Ok(manual::create_manual_match(invoice, payments, itinerary_payment_pairs))
}

// 生成报销表 PDF 命令
#[tauri::command]
async fn generate_form_pdf(
    match_results: Vec<MatchResult>,
    name: String,
    department: String,
    destination: String,
    travel_start: String,
    travel_end: String,
    companions: u32,
    hotel_level: String,
    output_path: String,
) -> Result<(), String> {
    let form = form_builder::build_reimbursement_form(
        &match_results,
        &name,
        &department,
        &destination,
        &travel_start,
        &travel_end,
        companions as usize,
        &hotel_level,
    );
    form_generator::generate_reimbursement_pdf(&form, &output_path).map_err(|e| e.to_string())
}

// 生成报销单 HTML 命令
#[tauri::command]
async fn generate_reimbursement_html(
    match_results: Vec<MatchResult>,
    name: String,
    department: String,
    destination: String,
    travel_start: String,
    travel_end: String,
    companions: u32,
    hotel_level: String,
    output_path: String,
) -> Result<String, String> {
    let form = form_builder::build_reimbursement_form(
        &match_results,
        &name,
        &department,
        &destination,
        &travel_start,
        &travel_end,
        companions as usize,
        &hotel_level,
    );
    form_html_generator::generate_reimbursement_html(&form, &output_path)
        .map_err(|e| e.to_string())?;
    Ok(output_path)
}

// 生成报销单 HTML 内容（不写文件，返回 HTML 字符串）
#[tauri::command]
async fn render_reimbursement_html(
    match_results: Vec<MatchResult>,
    name: String,
    department: String,
    destination: String,
    travel_start: String,
    travel_end: String,
    companions: u32,
    hotel_level: String,
) -> Result<String, String> {
    let form = form_builder::build_reimbursement_form(
        &match_results,
        &name,
        &department,
        &destination,
        &travel_start,
        &travel_end,
        companions as usize,
        &hotel_level,
    );
    Ok(form_html_generator::generate_reimbursement_html_string(&form))
}

// 预览报销表单（返回结构化数据，供前端实时显示可报销金额）
#[tauri::command]
async fn preview_reimbursement_form(
    match_results: Vec<MatchResult>,
    name: String,
    department: String,
    destination: String,
    travel_start: String,
    travel_end: String,
    companions: u32,
    hotel_level: String,
) -> Result<crate::models::reimbursement::ReimbursementForm, String> {
    Ok(form_builder::build_reimbursement_form(
        &match_results,
        &name,
        &department,
        &destination,
        &travel_start,
        &travel_end,
        companions as usize,
        &hotel_level,
    ))
}

// 生成报销单 Excel 命令
#[tauri::command]
async fn generate_reimbursement_xlsx(
    match_results: Vec<MatchResult>,
    name: String,
    department: String,
    destination: String,
    travel_start: String,
    travel_end: String,
    companions: u32,
    hotel_level: String,
    output_path: String,
) -> Result<String, String> {
    let form = form_builder::build_reimbursement_form(
        &match_results,
        &name,
        &department,
        &destination,
        &travel_start,
        &travel_end,
        companions as usize,
        &hotel_level,
    );
    form_xlsx_generator::generate_reimbursement_xlsx(&form, &match_results, &output_path)
        .map_err(|e| e.to_string())?;
    Ok(output_path)
}

// 全局导入结果
#[derive(serde::Serialize)]
struct GlobalImportResult {
    invoices: Vec<Invoice>,
    payments: Vec<PaymentRecord>,
    errors: Vec<[String; 2]>,
    /// 批次内去重命中的重复发票号列表
    duplicates: Vec<String>,
}

// 递归遍历目录收集所有文件
fn collect_files_recursive(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(rd) = dir.read_dir() {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_files_recursive(&path));
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    files
}

// 从混合路径列表中解析出所有支持的文件（展开目录、过滤扩展名）
// extensions 为空时默认使用发票相关扩展名
#[tauri::command]
fn collect_files(paths: Vec<String>, extensions: Option<Vec<String>>) -> Vec<String> {
    let default_exts = vec!["pdf".into(), "jpg".into(), "jpeg".into(), "png".into()];
    let exts = extensions.unwrap_or(default_exts);
    let mut result = Vec::new();

    for raw in &paths {
        let p = std::path::Path::new(raw);
        if p.is_dir() {
            for f in collect_files_recursive(p) {
                let ext = f.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if exts.iter().any(|e| e == &ext) {
                    result.push(f.to_string_lossy().to_string());
                }
            }
        } else if p.is_file() {
            let ext = p.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if exts.iter().any(|e| e == &ext) {
                result.push(raw.clone());
            }
        }
    }

    result
}

// 全局导入命令：选择文件夹后递归处理所有文件
#[tauri::command]
async fn batch_global_import(
    state: tauri::State<'_, AppState>,
    dir_path: String,
) -> Result<GlobalImportResult, String> {
    let dir = std::path::Path::new(&dir_path);
    if !dir.is_dir() {
        return Err(format!("路径不是有效目录: {}", dir_path));
    }

    let all_files = collect_files_recursive(dir);
    let mut invoices = Vec::new();
    let mut payments = Vec::new();
    let mut errors = Vec::new();

    let mut engine = state.ocr_engine.lock().await;

    let mut pdf_files = Vec::new();

    for path in &all_files {
        let path_str = path.to_string_lossy().to_string();
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "pdf" => {
                pdf_files.push(path_str);
            }
            "jpg" | "jpeg" | "png" => {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                match invoice_pipeline::parse_invoice_from_image(&path_str, &mut engine) {
                    Ok(inv) => invoices.push(inv),
                    Err(e) => errors.push([name, e]),
                }
            }
            "xlsx" | "xls" => {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                match parser::wechat_parser::parse_wechat_bill(&path_str) {
                    Ok(records) => payments.extend(records),
                    Err(e) => errors.push([name, e]),
                }
            }
            "csv" => {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                match parser::alipay_parser::parse_alipay_bill(&path_str) {
                    Ok(records) => payments.extend(records),
                    Err(e) => errors.push([name, e]),
                }
            }
            _ => {}
        }
    }

    // PDF 复用已有的 batch 解析逻辑（含发票→行程单→配对）
    let mut pdf_duplicates = Vec::new();
    if !pdf_files.is_empty() {
        let result = invoice_pipeline::parse_all_from_files(&pdf_files, &mut engine, &Default::default());
        invoices.extend(result.invoices);
        pdf_duplicates = result.duplicates;
        for (name, err) in result.errors {
            errors.push([name, err]);
        }
    }

    // 对合并后的全部发票（图片+PDF）再做一次批次内去重，
    // 覆盖图片之间、图片与 PDF 之间的重复
    let mut duplicates = pdf_duplicates;
    duplicates.extend(parser::dedup::deduplicate_invoices(&mut invoices));

    Ok(GlobalImportResult { invoices, payments, errors, duplicates })
}

// 生成对照单 HTML 命令
#[tauri::command]
async fn generate_comparison_html(
    state: tauri::State<'_, AppState>,
    match_results: Vec<MatchResult>,
    invoice_dir: String,
    output_dir: String,
) -> Result<String, String> {
    // 确保 PDFium 已初始化（生成图片需要）
    let _engine = state.ocr_engine.lock().await;
    comparison_html_generator::generate_comparison_html(
        &match_results,
        &invoice_dir,
        &output_dir,
        400,
    )
    .map_err(|e| e.to_string())
}

// 生成对照表 PDF 命令（原始发票页矢量嵌入 + 支付单号文字）
#[tauri::command]
async fn generate_comparison_image_pdf(
    state: tauri::State<'_, AppState>,
    match_results: Vec<MatchResult>,
    invoice_dir: String,
    output_path: String,
    destination: Option<String>,
) -> Result<(), String> {
    let _engine = state.ocr_engine.lock().await;
    comparison_image_pdf_generator::generate_comparison_image_pdf(
        &match_results,
        &invoice_dir,
        &output_path,
        destination.as_deref(),
    )
    .map_err(|e| e.to_string())
}

// 渲染 PDF/图片预览：返回所有页面的图片路径
#[tauri::command]
async fn render_pdf_preview(file_path: String) -> Result<Vec<String>, String> {
    use std::io::Read;
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("文件不存在: {}", file_path));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "bmp" | "tiff") {
        let mut file = std::fs::File::open(&file_path).map_err(|e| format!("无法打开文件: {}", e))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).map_err(|e| format!("读取文件失败: {}", e))?;
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
        let mime = if ext == "png" { "image/png" } else { "image/jpeg" };
        return Ok(vec![format!("data:{};base64,{}", mime, b64)]);
    }

    if ext == "pdf" {
        let tmp_dir = std::env::temp_dir()
            .join(format!("invoice_preview_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir)
            .map_err(|e| format!("无法创建临时目录: {}", e))?;
        let tmp = tmp_dir.to_string_lossy().to_string();
        let paths = crate::pdf::image_embedder::render_pdf_all_pages_to_pngs(&file_path, &tmp, 150)?;
        let mut results = Vec::new();
        for p in &paths {
            let mut file = std::fs::File::open(p).map_err(|e| format!("无法打开临时文件: {}", e))?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map_err(|e| format!("读取临时文件失败: {}", e))?;
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
            results.push(format!("data:image/png;base64,{}", b64));
        }
        std::fs::remove_dir_all(&tmp_dir).ok();
        return Ok(results);
    }

    Err(format!("不支持的文件类型: {}", ext))
}

// 调试：提取 PDF 文字并返回三引擎坐标对比数据
#[tauri::command]
async fn debug_extract_texts(
    state: tauri::State<'_, AppState>,
    file_path: String,
    dpi: Option<u32>,
) -> Result<crate::pdf::debug_extract::DebugTextResult, String> {
    let dpi = dpi.unwrap_or(200);
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("文件不存在: {}", file_path));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "pdf" {
        return Err(format!("调试界面仅支持 PDF，收到: .{}", ext));
    }
    // OCR 引擎可能未初始化（模型未下载），传 Some 让后端自行降级
    let mut engine = state.ocr_engine.lock().await;
    let engine_ref: Option<&mut OcrEngine> = if engine.health().unwrap_or(false) {
        Some(&mut *engine)
    } else {
        None
    };
    crate::pdf::debug_extract::debug_extract_texts(&file_path, dpi, engine_ref)
}

// 调用系统默认程序打开文件
#[tauri::command]
async fn open_file_with_system(file_path: String) -> Result<(), String> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("文件不存在: {}", file_path));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", &file_path])
            .spawn()
            .map_err(|e| format!("无法打开文件: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("无法打开文件: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("无法打开文件: {}", e))?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        return Err("不支持的操作系统".to_string());
    }
    Ok(())
}

// 生成对照表 PDF 命令
#[tauri::command]
async fn generate_comparison_pdf(
    match_results: Vec<MatchResult>,
    unmatched_invoice_ids: Vec<String>,
    unmatched_payment_ids: Vec<String>,
    output_path: String,
) -> Result<(), String> {
    comparison_generator::generate_comparison_pdf(
        &match_results,
        &unmatched_invoice_ids,
        &unmatched_payment_ids,
        &output_path,
    )
    .map_err(|e| e.to_string())
}

// 生成完整信息对比 Excel 命令
#[tauri::command]
async fn generate_comparison_xlsx(
    match_results: Vec<MatchResult>,
    output_path: String,
) -> Result<String, String> {
    comparison_xlsx_generator::generate_comparison_xlsx(&match_results, &output_path)
        .map_err(|e| e.to_string())?;
    Ok(output_path)
}

// 多趟出差自动分趟命令：origin 为 None 自动分趟，Some 时以指定出发城市全量重分
#[tauri::command]
fn segment_trips(
    match_results: Vec<MatchResult>,
    origin: Option<String>,
) -> Result<matching::segment::SegmentResult, String> {
    Ok(matching::segment::segment_trips(&match_results, origin.as_deref()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            ocr_engine: AsyncMutex::new(OcrEngine::uninitialized()),
        })
        .invoke_handler(tauri::generate_handler![
            ocr_health,
            ocr_recognize_image,
            ocr_recognize_pdf,
            download_ocr_models,
            get_ocr_model_config,
            set_ocr_model_config,
            get_reimbursement_config,
            set_reimbursement_config,
            get_builtin_hotel_standards,
            recognize_invoice,
            batch_recognize,
            recognize_itinerary,
            import_wechat_bill,
            import_alipay_bill,
            import_bill,
            auto_match,
            manual_match,
            generate_form_pdf,
            generate_comparison_pdf,
            generate_comparison_xlsx,
            generate_comparison_image_pdf,
            generate_comparison_html,
            generate_reimbursement_html,
            render_reimbursement_html,
            preview_reimbursement_form,
            generate_reimbursement_xlsx,
            collect_files,
            batch_global_import,
            render_pdf_preview,
            debug_extract_texts,
            open_file_with_system,
            segment_trips,

        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // 初始化 OCR 引擎：优先用户下载的模型，其次打包内置模型
            let engine = match ocr::model_downloader::find_models_dir(&app_handle) {
                Some(dir) => {
                    let dir_str = dir.to_string_lossy().to_string();
                    match OcrEngine::new(&dir_str) {
                        Ok(e) => {
                            if e.health().unwrap_or(false) {
                                println!("OCR engine initialized from: {}", dir_str);
                            }
                            e
                        }
                        Err(e) => {
                            eprintln!("Warning: OCR init failed ({}): {}", dir_str, e);
                            OcrEngine::uninitialized()
                        }
                    }
                }
                None => {
                    eprintln!("OCR models not found. 可在首页点击「下载OCR模型」在线下载。");
                    OcrEngine::uninitialized()
                }
            };

            let state = app.state::<AppState>();
            *state.ocr_engine.blocking_lock() = engine;

            // 加载报销标准配置到进程内全局状态（启动时生效）
            let cfg = reimbursement_config::load_config(&app_handle);
            reimbursement_config::apply_config(&cfg);

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
