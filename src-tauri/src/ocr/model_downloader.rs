use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

/// 默认模型下载基址（GitHub Releases，可在前端设置中修改）
const DEFAULT_MODEL_BASE_URL: &str =
    "https://github.com/chenfuxu920/invoice-reimbursement/releases/download/ocr-models-v1";

/// OCR 运行所需的三个模型文件
const MODEL_FILES: &[&str] = &[
    "PP-OCRv5_mobile_det.mnn",
    "PP-OCRv5_mobile_rec.mnn",
    "ppocr_keys_v5.txt",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrModelConfig {
    pub model_base_url: String,
}

impl Default for OcrModelConfig {
    fn default() -> Self {
        Self {
            model_base_url: DEFAULT_MODEL_BASE_URL.to_string(),
        }
    }
}

/// 下载模型的存放目录：app_data_dir/models
pub fn get_models_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("models"))
}

/// 查找可用的模型目录：优先用户下载的，其次打包内置的（resource_dir/models）
pub fn find_models_dir(app: &AppHandle) -> Option<PathBuf> {
    // 1. 用户下载的模型
    if let Some(dir) = get_models_dir(app) {
        if dir.join(MODEL_FILES[0]).exists() {
            return Some(dir);
        }
    }
    // 2. 打包内置模型
    let resource = app.path().resource_dir().ok()?;
    let bundled = resource.join("models");
    if bundled.join(MODEL_FILES[0]).exists() {
        return Some(bundled);
    }
    None
}

/// 读取 OCR 配置（文件不存在或解析失败则返回默认）
pub fn load_config(app: &AppHandle) -> OcrModelConfig {
    let Some(dir) = app.path().app_data_dir().ok() else {
        return OcrModelConfig::default();
    };
    let path = dir.join("ocr-config.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 保存 OCR 配置
pub fn save_config(app: &AppHandle, config: &OcrModelConfig) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("ocr-config.json"), json).map_err(|e| e.to_string())
}

/// 下载 OCR 模型文件到 app_data_dir/models/，返回模型目录路径。
/// 通过 `ocr-download-progress` 事件报告进度，完成后发 `ocr-download-complete`。
pub async fn download_models(app: &AppHandle) -> Result<PathBuf, String> {
    let config = load_config(app);
    let base_url = config.model_base_url.trim_end_matches('/');
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let models_dir = app_data.join("models");
    std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;

    let total = MODEL_FILES.len();
    for (i, file) in MODEL_FILES.iter().enumerate() {
        let _ = app.emit(
            "ocr-download-progress",
            serde_json::json!({ "file": file, "index": i, "total": total }),
        );

        let url = format!("{}/{}", base_url, file);
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("下载 {} 失败: {}", file, e))?;
        if !resp.status().is_success() {
            return Err(format!(
                "下载 {} 失败: HTTP {}（请检查下载地址是否正确）",
                file,
                resp.status()
            ));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("读取 {} 失败: {}", file, e))?;
        std::fs::write(models_dir.join(file), &bytes)
            .map_err(|e| format!("写入 {} 失败: {}", file, e))?;
    }

    let _ = app.emit("ocr-download-complete", ());
    Ok(models_dir)
}
