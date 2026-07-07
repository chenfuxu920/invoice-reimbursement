use std::path::{Path, PathBuf};

fn ascii_safe(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            s.push(c);
        } else {
            s.push('_');
        }
    }
    s
}

/// 使用 zpdf（纯 Rust）渲染 PDF 所有页面为 RgbImage，无需 pdfium.dll
pub fn render_pdf_to_rgb_images(pdf_path: &str, dpi: u32) -> Result<Vec<image::RgbImage>, String> {
    use zpdf::{ContentInterpreter, ImageCache, PdfDocument, RenderBackend};

    let data = std::fs::read(pdf_path).map_err(|e| format!("读取 PDF 失败: {}", e))?;
    let doc = PdfDocument::open(data).map_err(|e| format!("解析 PDF 失败: {:?}", e))?;

    let scale = dpi as f32 / 72.0;
    let page_count = doc.page_count();
    let mut images = Vec::new();

    for i in 0..page_count {
        let page = doc
            .page(i)
            .map_err(|e| format!("获取页面 {} 失败: {:?}", i, e))?;
        let mut fonts = doc.load_page_fonts(&page);
        let mut img_cache = ImageCache::new();
        let content = doc
            .page_content_bytes(&page)
            .map_err(|e| format!("获取页面 {} 内容失败: {:?}", i, e))?;

        let display_list = ContentInterpreter::new(page.effective_box())
            .with_page_rotation(page.rotate)
            .with_fonts(&mut fonts)
            .with_document(doc.file(), &page.resources)
            .with_images(&mut img_cache)
            .interpret(&content);

        let mut renderer = zpdf::cpu::CpuRenderer::new()
            .with_fonts(&fonts)
            .with_images(&img_cache);
        let rendered = renderer
            .render_display_list(&display_list, scale)
            .map_err(|e| format!("渲染页面 {} 失败: {:?}", i, e))?;

        // RGBA → RGB
        let rgb_data: Vec<u8> = rendered
            .data
            .chunks_exact(4)
            .flat_map(|px| &px[..3])
            .copied()
            .collect();
        let img = image::RgbImage::from_raw(rendered.width, rendered.height, rgb_data)
            .ok_or_else(|| {
                format!(
                    "创建图片失败 ({}x{})",
                    rendered.width, rendered.height
                )
            })?;
        images.push(img);
    }

    Ok(images)
}

pub fn render_pdf_page_to_png(
    pdf_path: &str,
    page_index: u32,
    output_dir: &str,
    dpi: u32,
) -> Result<PathBuf, String> {
    let images = render_pdf_to_rgb_images(pdf_path, dpi)?;
    let img = images
        .get(page_index as usize)
        .ok_or_else(|| format!("页面 {} 不存在（共 {} 页）", page_index, images.len()))?;

    let stem = ascii_safe(
        &Path::new(pdf_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy(),
    );
    let output_path = Path::new(output_dir).join(format!("{stem}_p{page_index}.png"));
    img.save(&output_path)
        .map_err(|e| format!("保存 PNG 失败: {:?}", e))?;
    Ok(output_path)
}

pub fn render_pdf_all_pages_to_pngs(
    pdf_path: &str,
    output_dir: &str,
    dpi: u32,
) -> Result<Vec<PathBuf>, String> {
    let images = render_pdf_to_rgb_images(pdf_path, dpi)?;
    let stem = ascii_safe(
        &Path::new(pdf_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy(),
    );

    let mut result = Vec::new();
    for (i, img) in images.iter().enumerate() {
        let output_path = Path::new(output_dir).join(format!("{stem}_p{i}.png"));
        img.save(&output_path)
            .map_err(|e| format!("保存 PNG 失败: {:?}", e))?;
        result.push(output_path);
    }
    Ok(result)
}

pub fn is_supported_image(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "bmp" | "tiff")
}
