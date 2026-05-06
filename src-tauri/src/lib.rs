pub mod matching;
pub mod models;
pub mod ocr;
pub mod parser;
pub mod pdf;

use ocr::{OcrEngine, OcrTextItem};
use parser::invoice_parser::parse_invoice_text;
use parser::itinerary_parser::parse_itinerary_text;
use parser::wechat_parser;
use parser::alipay_parser;
use models::invoice::{Invoice, InvoiceSource, Itinerary};
use models::payment::PaymentRecord;
use models::match_result::MatchResult;
use matching::batch;
use matching::manual;
use crate::pdf::form_generator;
use crate::pdf::comparison_generator;
use crate::pdf::form_builder;
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

    let source = if file_type == "pdf" {
        InvoiceSource::Pdf(file_path.clone())
    } else {
        InvoiceSource::Photo(file_path.clone())
    };

    let result = if file_type == "pdf" {
        let resp = engine.recognize_pdf(&file_path)?;
        let all_texts: Vec<OcrTextItem> = resp.pages.iter().flat_map(|p| p.texts.clone()).collect();
        parse_invoice_text(&all_texts, source)?
    } else {
        let resp = engine.recognize_image(&file_path)?;
        parse_invoice_text(&resp.texts, source)?
    };

    Ok(result)
}

// 行程单识别与解析命令
#[tauri::command]
async fn recognize_itinerary(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<Vec<Itinerary>, String> {
    let mut engine = state.ocr_engine.lock().await;
    let engine = engine
        .as_mut()
        .ok_or("OCR engine not initialized. Model files may be missing.")?;
    let resp = engine.recognize_pdf(&file_path)?;
    let all_texts: Vec<OcrTextItem> = resp.pages.iter().flat_map(|p| p.texts.clone()).collect();
    Ok(parse_itinerary_text(&all_texts))
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
    travel_start: String,
    travel_end: String,
    companions: u32,
    output_path: String,
) -> Result<(), String> {
    let form = form_builder::build_reimbursement_form(
        &match_results,
        &name,
        &department,
        &travel_start,
        &travel_end,
        companions as usize,
    );
    form_generator::generate_reimbursement_pdf(&form, &output_path).map_err(|e| e.to_string())
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
            recognize_itinerary,
            import_wechat_bill,
            import_alipay_bill,
            auto_match,
            manual_match,
            generate_form_pdf,
            generate_comparison_pdf,
        ])
        .setup(|app| {
            // 初始化嵌入式 OCR 引擎
            let resource_dir = app
                .path()
                .resource_dir()
                .expect("Failed to get resource directory");
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
