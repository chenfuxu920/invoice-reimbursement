/// 解析发票链接，提取发票信息
/// 支持的链接格式：
/// - 全国增值税发票查验平台链接
/// - 电子发票短链接
/// - 二维码中的链接
use reqwest::Client;

pub async fn fetch_invoice_from_link(url: &str) -> Result<String, String> {
    let client = Client::new();
    let resp = client.get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let html = resp.text().await.map_err(|e| e.to_string())?;
    Ok(html)
}

/// 从二维码图片中提取发票链接
/// TODO: 实现二维码解码
pub fn extract_url_from_qrcode(_image_path: &str) -> Result<String, String> {
    Err("二维码解析功能待实现".to_string())
}
