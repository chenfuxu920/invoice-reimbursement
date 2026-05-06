use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcrTextItem {
    pub text: String,
    pub confidence: f64,
    pub box_coords: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcrImageResponse {
    pub texts: Vec<OcrTextItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcrPageResult {
    pub page: u32,
    pub texts: Vec<OcrTextItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcrPdfResponse {
    pub pages: Vec<OcrPageResult>,
}

pub struct OcrClient {
    client: Client,
    base_url: String,
}

impl OcrClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    pub async fn health(&self) -> Result<bool, String> {
        let resp = self.client
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        Ok(resp.status().is_success())
    }

    pub async fn recognize_image(&self, file_path: &str) -> Result<OcrImageResponse, String> {
        let file_bytes = std::fs::read(file_path).map_err(|e| e.to_string())?;
        let file_name = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("image/png")
            .map_err(|e| e.to_string())?;

        let form = reqwest::multipart::Form::new()
            .part("file", part);

        let resp = self.client
            .post(format!("{}/ocr/image", self.base_url))
            .multipart(form)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        resp.json::<OcrImageResponse>()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn recognize_pdf(&self, file_path: &str) -> Result<OcrPdfResponse, String> {
        let file_bytes = std::fs::read(file_path).map_err(|e| e.to_string())?;
        let file_name = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("application/pdf")
            .map_err(|e| e.to_string())?;

        let form = reqwest::multipart::Form::new()
            .part("file", part);

        let resp = self.client
            .post(format!("{}/ocr/pdf", self.base_url))
            .multipart(form)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        resp.json::<OcrPdfResponse>()
            .await
            .map_err(|e| e.to_string())
    }
}
