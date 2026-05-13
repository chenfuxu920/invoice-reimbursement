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

pub fn render_pdf_page_to_png(
    pdf_path: &str,
    page_index: u32,
    output_dir: &str,
    dpi: u32,
) -> Result<PathBuf, String> {
    use pdfium_render::prelude::*;

    let lib_path = crate::ocr::engine::get_pdfium_path()
        .ok_or_else(|| "PDFium not initialized".to_string())?;

    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(lib_path)
            .map_err(|e| format!("Failed to bind PDFium: {:?}", e))?,
    );

    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| format!("Failed to load PDF: {:?}", e))?;

    let page = document
        .pages()
        .get(page_index as u16)
        .map_err(|e| format!("Failed to get page {}: {:?}", page_index, e))?;

    render_page_to_png_impl(&page, pdf_path, page_index, output_dir, dpi)
}

pub fn render_pdf_all_pages_to_pngs(
    pdf_path: &str,
    output_dir: &str,
    dpi: u32,
) -> Result<Vec<PathBuf>, String> {
    use pdfium_render::prelude::*;

    let lib_path = crate::ocr::engine::get_pdfium_path()
        .ok_or_else(|| "PDFium not initialized".to_string())?;

    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(lib_path)
            .map_err(|e| format!("Failed to bind PDFium: {:?}", e))?,
    );

    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| format!("Failed to load PDF: {:?}", e))?;

    let mut result = Vec::new();
    for i in 0..document.pages().len() {
        let page = document
            .pages()
            .get(i as u16)
            .map_err(|e| format!("Failed to get page {}: {:?}", i, e))?;
        let img_path = render_page_to_png_impl(&page, pdf_path, i as u32, output_dir, dpi)?;
        result.push(img_path);
    }

    Ok(result)
}

fn render_page_to_png_impl(
    page: &pdfium_render::prelude::PdfPage,
    pdf_path: &str,
    page_index: u32,
    output_dir: &str,
    dpi: u32,
) -> Result<PathBuf, String> {
    use pdfium_render::prelude::*;

    let scale = dpi as f32 / 72.0;
    let target_width = (page.width().value as f32 * scale) as i32;
    let target_height = (page.height().value as f32 * scale) as i32;

    let bitmap = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(target_width)
                .set_target_height(target_height)
                .render_form_data(false),
        )
        .map_err(|e| format!("Failed to render PDF page: {:?}", e))?;

    let raw_pixels = bitmap.as_raw_bytes();
    let img_width = bitmap.width() as u32;
    let img_height = bitmap.height() as u32;
    let stride = raw_pixels.len() / img_height as usize;

    let mut rgb_data = Vec::with_capacity((img_width * img_height * 3) as usize);
    for y in 0..img_height {
        let row_start = y as usize * stride;
        for x in 0..img_width {
            let px = row_start + (x as usize) * 4;
            if px + 3 < raw_pixels.len() {
                rgb_data.push(raw_pixels[px]);
                rgb_data.push(raw_pixels[px + 1]);
                rgb_data.push(raw_pixels[px + 2]);
            }
        }
    }

    let img = image::RgbImage::from_raw(img_width, img_height, rgb_data)
        .ok_or("Failed to create image from RGB data")?;

    let stem = ascii_safe(&Path::new(pdf_path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy());
    let output_path = Path::new(output_dir).join(format!("{stem}_p{page_index}.png"));
    img.save(&output_path)
        .map_err(|e| format!("Failed to save PNG: {:?}", e))
        .map(|_| output_path)
}

pub fn is_supported_image(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "bmp" | "tiff")
}
