mod matching;
mod models;
mod ocr;
mod parser;

use ocr::{OcrClient, OcrServiceManager, OcrTextItem};
use parser::invoice_parser::parse_invoice_text;
use parser::itinerary_parser::parse_itinerary_text;
use parser::wechat_parser;
use parser::alipay_parser;
use models::invoice::{Invoice, InvoiceSource, Itinerary};
use models::payment::PaymentRecord;
use models::match_result::MatchResult;
use matching::batch;
use matching::manual;
use std::sync::Mutex;
use tokio::sync::Mutex as AsyncMutex;
use tauri::Manager;

// 应用状态
struct AppState {
    ocr_client: AsyncMutex<OcrClient>,
    ocr_service: Mutex<OcrServiceManager>,
}

// OCR 健康检查命令
#[tauri::command]
async fn ocr_health(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let client = state.ocr_client.lock().await;
    client.health().await
}

// OCR 图片识别命令
#[tauri::command]
async fn ocr_recognize_image(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<serde_json::Value, String> {
    let client = state.ocr_client.lock().await;
    let result = client.recognize_image(&file_path).await?;
    Ok(serde_json::to_value(result).map_err(|e| e.to_string())?)
}

// OCR PDF 识别命令
#[tauri::command]
async fn ocr_recognize_pdf(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<serde_json::Value, String> {
    let client = state.ocr_client.lock().await;
    let result = client.recognize_pdf(&file_path).await?;
    Ok(serde_json::to_value(result).map_err(|e| e.to_string())?)
}

// 启动 OCR 服务命令
#[tauri::command]
async fn start_ocr_service(
    state: tauri::State<'_, AppState>,
    project_dir: String,
) -> Result<(), String> {
    let mut service = state.ocr_service.lock().map_err(|e| e.to_string())?;
    service.start(&project_dir)
}

// 停止 OCR 服务命令
#[tauri::command]
async fn stop_ocr_service(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut service = state.ocr_service.lock().map_err(|e| e.to_string())?;
    service.stop()
}

// 检查 OCR 服务是否运行
#[tauri::command]
async fn is_ocr_service_running(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let service = state.ocr_service.lock().map_err(|e| e.to_string())?;
    Ok(service.is_running())
}

// 发票识别与解析命令
#[tauri::command]
async fn recognize_invoice(
    state: tauri::State<'_, AppState>,
    file_path: String,
    file_type: String,  // "image" | "pdf"
) -> Result<Invoice, String> {
    let client = state.ocr_client.lock().await;

    let source = if file_type == "pdf" {
        InvoiceSource::Pdf(file_path.clone())
    } else {
        InvoiceSource::Photo(file_path.clone())
    };

    let result = if file_type == "pdf" {
        let resp = client.recognize_pdf(&file_path).await?;
        let all_texts: Vec<OcrTextItem> = resp.pages.iter()
            .flat_map(|p| p.texts.clone())
            .collect();
        parse_invoice_text(&all_texts, source)?
    } else {
        let resp = client.recognize_image(&file_path).await?;
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
    let client = state.ocr_client.lock().await;
    let resp = client.recognize_pdf(&file_path).await?;
    let all_texts: Vec<OcrTextItem> = resp.pages.iter()
        .flat_map(|p| p.texts.clone())
        .collect();
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            ocr_client: AsyncMutex::new(OcrClient::new("http://127.0.0.1:8080")),
            ocr_service: Mutex::new(OcrServiceManager::new()),
        })
        .invoke_handler(tauri::generate_handler![
            ocr_health,
            ocr_recognize_image,
            ocr_recognize_pdf,
            start_ocr_service,
            stop_ocr_service,
            is_ocr_service_running,
            recognize_invoice,
            recognize_itinerary,
            import_wechat_bill,
            import_alipay_bill,
            auto_match,
            manual_match,
        ])
        .setup(|app| {
            // 应用启动时自动启动 OCR 服务
            let state = app.state::<AppState>();
            let service = state.ocr_service.lock().unwrap();
            // 获取项目目录（使用当前工作目录或可配置路径）
            if let Ok(project_dir) = std::env::current_dir() {
                let project_dir_str = project_dir.to_string_lossy().to_string();
                drop(service); // 释放锁后再启动
                let state = app.state::<AppState>();
                let mut service = state.ocr_service.lock().unwrap();
                if let Err(e) = service.start(&project_dir_str) {
                    eprintln!("Warning: Failed to auto-start OCR service: {}", e);
                } else {
                    println!("OCR service auto-started successfully");
                }
            }

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 应用窗口关闭时自动停止 OCR 服务
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<AppState>();
                let mut service = state.ocr_service.lock().unwrap();
                if let Err(e) = service.stop() {
                    eprintln!("Warning: Failed to stop OCR service on close: {}", e);
                } else {
                    println!("OCR service stopped on application close");
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
