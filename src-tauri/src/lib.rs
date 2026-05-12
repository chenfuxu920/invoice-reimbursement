pub mod matching;
pub mod models;
pub mod ocr;
pub mod parser;
pub mod pdf;

use ocr::{OcrEngine, OcrTextItem};
use parser::itinerary_parser::parse_itinerary_text;
use parser::wechat_parser;
use parser::alipay_parser;
use models::invoice::{Invoice, Itinerary};
use models::payment::PaymentRecord;
use models::match_result::MatchResult;
use matching::batch;
use matching::manual;
use crate::pdf::form_generator;
use crate::pdf::comparison_generator;
use crate::pdf::form_builder;
use crate::pdf::form_html_generator;
use crate::pdf::invoice_pipeline::{self, ParseResult};
use crate::pdf::text_extractor;
use tokio::sync::Mutex as AsyncMutex;
use tauri::Manager;

// 应用状态
struct AppState {
    ocr_engine: AsyncMutex<Option<OcrEngine>>,
}

// OCR 健康检查命令
#[tauri::command]
async fn ocr_health(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let engine = state.ocr_engine.lock().await;
    match engine.as_ref() {
        Some(e) => Ok(e.health()?),
        None => Ok(false),
    }
}

// OCR 图片识别命令
#[tauri::command]
async fn ocr_recognize_image(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<serde_json::Value, String> {
    let mut engine = state.ocr_engine.lock().await;
    let engine = engine
        .as_mut()
        .ok_or("OCR engine not initialized. Model files may be missing.")?;
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
    let engine = engine
        .as_mut()
        .ok_or("OCR engine not initialized. Model files may be missing.")?;
    let result = engine.recognize_pdf(&file_path)?;
    Ok(serde_json::to_value(result).map_err(|e| e.to_string())?)
}

// 发票识别与解析命令
#[tauri::command]
async fn recognize_invoice(
    state: tauri::State<'_, AppState>,
    file_path: String,
    file_type: String, // "image" | "pdf"
) -> Result<Invoice, String> {
    let mut engine = state.ocr_engine.lock().await;
    let engine = engine
        .as_mut()
        .ok_or("OCR engine not initialized. Model files may be missing.")?;
    if file_type == "pdf" {
        invoice_pipeline::parse_invoice_from_pdf(&file_path, engine)
    } else {
        invoice_pipeline::parse_invoice_from_image(&file_path, engine)
    }
}

// 批量识别文件（发票+行程单），自动匹配行程单到发票
#[tauri::command]
async fn batch_recognize(
    state: tauri::State<'_, AppState>,
    file_paths: Vec<String>,
) -> Result<ParseResult, String> {
    let mut engine = state.ocr_engine.lock().await;
    let engine = engine
        .as_mut()
        .ok_or("OCR engine not initialized. Model files may be missing.")?;
    Ok(invoice_pipeline::parse_all_from_files(&file_paths, engine))
}

// 行程单识别与解析命令
#[tauri::command]
async fn recognize_itinerary(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<Vec<Itinerary>, String> {
    // 优先尝试直接提取 PDF 文字（适用于文字型 PDF）
    match text_extractor::extract_text_from_pdf(&file_path) {
        Ok(text_items) if text_extractor::has_sufficient_text(&text_items, 20) => {
            // 文字型 PDF，直接使用提取的文字
            Ok(parse_itinerary_text(&text_items))
        }
        _ => {
            // 扫描型 PDF 或文字提取失败，回退到 OCR
            let mut engine = state.ocr_engine.lock().await;
            let engine = engine
                .as_mut()
                .ok_or("OCR engine not initialized. Model files may be missing.")?;
            let resp = engine.recognize_pdf(&file_path)?;
            let all_texts: Vec<OcrTextItem> =
                resp.pages.iter().flat_map(|p| p.texts.clone()).collect();
            Ok(parse_itinerary_text(&all_texts))
        }
    }
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
) -> Result<MatchResult, String> {
    Ok(manual::create_manual_match(invoice, payments))
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            ocr_engine: AsyncMutex::new(None), // 将在 setup 中初始化
        })
        .invoke_handler(tauri::generate_handler![
            ocr_health,
            ocr_recognize_image,
            ocr_recognize_pdf,
            recognize_invoice,
            batch_recognize,
            recognize_itinerary,
            import_wechat_bill,
            import_alipay_bill,
            auto_match,
            manual_match,
            generate_form_pdf,
            generate_comparison_pdf,
            generate_reimbursement_html,
            render_reimbursement_html,
        ])
        .setup(|app| {
            // 初始化 PDFium（从资源目录加载 pdfium.dll 用于 PDF 渲染）
            let resource_dir = app
                .path()
                .resource_dir()
                .expect("Failed to get resource directory");
            let pdfium_dll = resource_dir.join("pdfium.dll");
            match ocr::engine::init_pdfium(&pdfium_dll) {
                Ok(()) => println!("PDFium initialized successfully from: {}", pdfium_dll.display()),
                Err(e) => eprintln!("Warning: PDFium init failed: {}", e),
            }

            // 初始化嵌入式 OCR 引擎
            let models_dir = resource_dir.join("models");
            let models_dir_str = models_dir.to_string_lossy().to_string();

            let engine = match OcrEngine::new(&models_dir_str) {
                Ok(e) => {
                    println!("OCR engine initialized successfully from: {}", models_dir_str);
                    Some(e)
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to initialize OCR engine ({}): {}",
                        models_dir_str, e
                    );
                    eprintln!("OCR features will be unavailable. Please ensure model files are in the 'models/' directory.");
                    None
                }
            };

            let state = app.state::<AppState>();
            *state.ocr_engine.blocking_lock() = engine;

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
