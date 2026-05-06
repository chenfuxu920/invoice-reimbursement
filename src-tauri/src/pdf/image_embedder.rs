use std::error::Error;
use std::path::Path;

/// 嵌入发票图片到 PDF
/// 当前为占位实现，后续可使用 printpdf 实现
pub fn embed_invoice_images(
    _image_paths: &[String],
    _output_path: &str,
) -> Result<(), Box<dyn Error>> {
    // TODO: 使用 printpdf 实现图片嵌入
    // genpdf 不支持图片嵌入，需要直接使用 printpdf 库
    Ok(())
}

/// 将多个图片合并为一个 PDF
pub fn images_to_pdf(
    _image_paths: &[String],
    _output_path: &str,
) -> Result<(), Box<dyn Error>> {
    // TODO: 实现
    Ok(())
}

/// 检查文件是否为支持的图片格式
pub fn is_supported_image(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "bmp" | "tiff")
}
