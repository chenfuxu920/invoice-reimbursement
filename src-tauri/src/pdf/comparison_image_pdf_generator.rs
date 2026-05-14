use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use ::image::GenericImageView;
use printpdf::*;

use crate::models::hotel_standard::get_hotel_nightly_rate_std;
use crate::models::invoice::{InvoiceCategory, InvoiceSource};
use crate::models::match_result::MatchResult;
enum PdfBlock {
    Invoice {
        img: PathBuf,
        payment: String,
        over_std: Option<String>,
    },
    ItineraryPage {
        img: PathBuf,
    },
    ItineraryTable {
        rows: Vec<(usize, f64, String)>,
    },
}

fn load_printpdf_font(doc: &PdfDocumentReference) -> Result<IndirectFontRef, Box<dyn Error>> {
    let candidates: Vec<&str> = if cfg!(target_os = "windows") {
        vec![
            "C:/Windows/Fonts/simsun.ttc",
            "C:/Windows/Fonts/simhei.ttf",
            "C:/Windows/Fonts/msyh.ttc",
        ]
    } else {
        vec![
            "/usr/share/fonts/truetype/noto-cjk/NotoSansSC-Regular.ttf",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ]
    };
    for path in candidates {
        if Path::new(path).exists() {
            let file = fs::File::open(path)?;
            return Ok(doc.add_external_font(file)?);
        }
    }
    Ok(doc.add_builtin_font(BuiltinFont::Helvetica)?)
}

pub fn generate_comparison_image_pdf(
    match_results: &[MatchResult],
    invoice_dir: &str,
    output_path: &str,
    dpi: u32,
    destination: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let output = Path::new(output_path);
    let parent = output.parent().unwrap_or(Path::new("."));
    let tmp_dir = parent.join("__pdf_tmp");
    fs::create_dir_all(&tmp_dir)?;



    let mut blocks: Vec<PdfBlock> = Vec::new();

    for result in match_results {
        if let InvoiceSource::Pdf(pdf_path) = &result.invoice.source {
            let is_virtual = result.invoice.invoice_number.is_empty()
                && matches!(result.invoice.category, InvoiceCategory::CityTransport)
                && !result.invoice.itineraries.is_empty();
            if !is_virtual {
                let tmp = tmp_dir.to_string_lossy().to_string();
                let img_path =
                    super::image_embedder::render_pdf_page_to_png(pdf_path, 0, &tmp, dpi)?;

                let nightly_rate_std = destination.and_then(|_| {
                    if matches!(result.invoice.category, InvoiceCategory::Hotel) {
                        destination.map(get_hotel_nightly_rate_std)
                    } else {
                        None
                    }
                });
                let over_std = nightly_rate_std.and_then(|std| {
                    result.invoice.hotel_detail.as_ref().map(|detail| {
                        let standard_amount = std * detail.nights as f64;
                        if result.invoice.amount > standard_amount + 0.01 {
                            Some(format!("发票金额{:.2}，实报{:.2}", result.invoice.amount, standard_amount))
                        } else {
                            None
                        }
                    }).flatten()
                });

                let has_table = matches!(result.invoice.category, InvoiceCategory::CityTransport)
                    && !result.invoice.itineraries.is_empty();
                let payment_text = if has_table {
                    String::new()
                } else if result.payments.len() == 1 {
                    let p = &result.payments[0];
                    let prefix = match p.source {
                        crate::models::payment::PaymentSource::Wechat => "\u{5fae}\u{4fe1}\u{5355}\u{53f7}\u{ff1a}",
                        crate::models::payment::PaymentSource::Alipay => "\u{652f}\u{4ed8}\u{5b9d}\u{5355}\u{53f7}\u{ff1a}",
                    };
                    format!("{}{}", prefix, p.transaction_id)
                } else {
                    result
                        .payments
                        .iter()
                        .map(|p| {
                            let prefix = match p.source {
                                crate::models::payment::PaymentSource::Wechat => "\u{5fae}\u{4fe1}\u{5355}\u{53f7}\u{ff1a}",
                                crate::models::payment::PaymentSource::Alipay => "\u{652f}\u{4ed8}\u{5b9d}\u{5355}\u{53f7}\u{ff1a}",
                            };
                            format!("{}{}", prefix, p.transaction_id)
                        })
                        .collect::<Vec<_>>()
                        .join("\u{ff0c}")
                };
                blocks.push(PdfBlock::Invoice {
                    img: img_path,
                    payment: payment_text,
                    over_std,
                });
            }
        }

        if matches!(result.invoice.category, InvoiceCategory::CityTransport)
            && !result.invoice.itineraries.is_empty()
        {
            let mut itinerary_imgs: Vec<PathBuf> = Vec::new();
            if let Some(filename) = &result.invoice.itinerary_file {
                let pdf_path = Path::new(invoice_dir).join(filename);
                let pdf_str = pdf_path.to_string_lossy().to_string();
                if pdf_path.exists() {
                    let tmp = tmp_dir.to_string_lossy().to_string();
                    let imgs =
                        super::image_embedder::render_pdf_all_pages_to_pngs(&pdf_str, &tmp, dpi)?;
                    itinerary_imgs = imgs;
                }
            }

            for img in itinerary_imgs {
                blocks.push(PdfBlock::ItineraryPage { img });
            }

            let mut rows: Vec<(usize, f64, String)> = Vec::new();
            for (i, itin) in result.invoice.itineraries.iter().enumerate() {
                let pay_id = result
                    .payments
                    .get(i)
                    .map(|p| p.transaction_id.clone())
                    .unwrap_or_default();
                rows.push((i + 1, itin.amount, pay_id));
            }

            if !rows.is_empty() {
                blocks.push(PdfBlock::ItineraryTable { rows });
            }
        }
    }

    let page_w = Mm(297.0);
    let page_h = Mm(210.0);
    let (doc, page1_idx, layer1_idx) =
        PdfDocument::new("\u{53d1}\u{7968}\u{5bf9}\u{7167}\u{5355}", page_w, page_h, "Layer 1");

    let font = load_printpdf_font(&doc)?;

    fn render_block_body(
        _doc: &PdfDocumentReference,
        layer: &PdfLayerReference,
        block: &PdfBlock,
        font: &IndirectFontRef,
        page_w: Mm,
        page_h: Mm,
    ) -> Result<(), Box<dyn Error>> {
        match block {
            PdfBlock::Invoice { img, payment, over_std } => {
                let img_data = ::image::open(img)?;
                let (img_w, img_h) = img_data.dimensions();

                let margin = 8.0;
                let bottom_text_h = if over_std.is_some() { 35.0 } else { 20.0 };
                let max_w = page_w.0 - margin - margin;
                let max_h = page_h.0 - margin - margin - bottom_text_h;

                let dpi_factor = 72.0 / 25.4;
                let scale = ((max_w * dpi_factor) / img_w.max(1) as f32)
                    .min((max_h * dpi_factor) / img_h.max(1) as f32);

                let final_w = img_w as f32 * scale * (25.4 / 72.0);
                let final_h = img_h as f32 * scale * (25.4 / 72.0);
                let x = (page_w.0 - final_w) / 2.0;
                let y = (page_h.0 - final_h) / 2.0 + 5.0;

                let rgba = img_data.to_rgba8();
                let (w, h) = rgba.dimensions();
                let mut rgb_data = Vec::with_capacity((w * h * 3) as usize);
                for y in 0..h {
                    for x in 0..w {
                        let px = rgba.get_pixel(x, y);
                        rgb_data.push(px[0]);
                        rgb_data.push(px[1]);
                        rgb_data.push(px[2]);
                    }
                }
                let image_xobj = ImageXObject {
                    width: Px(w as usize),
                    height: Px(h as usize),
                    color_space: ColorSpace::Rgb,
                    bits_per_component: ColorBits::Bit8,
                    interpolate: true,
                    image_data: rgb_data,
                    image_filter: None,
                    smask: None,
                    clipping_bbox: None,
                };
                let image = Image::from(image_xobj);
                image.add_to_layer(
                    layer.clone(),
                    ImageTransform {
                        translate_x: Some(Mm(x)),
                        translate_y: Some(Mm(y)),
                        scale_x: Some(final_w / (w as f32 * (25.4 / 72.0))),
                        scale_y: Some(final_h / (h as f32 * (25.4 / 72.0))),
                        rotate: None,
                        dpi: Some(72.0),
                    },
                );

                if !payment.is_empty() || over_std.is_some() {
                    if let Some(ref over) = over_std {
                        let font_size = 14.0;
                        let text_w_mm = over.len() as f32 * font_size * (25.4 / 72.0) * 0.55;
                        let cx = (page_w.0 - text_w_mm) / 2.0;
                        layer.use_text(over.as_str(), font_size, Mm(cx), Mm(12.0), font);
                    }
                    if !payment.is_empty() {
                        let font_size = 14.0;
                        let text_w_mm = payment.len() as f32 * font_size * (25.4 / 72.0) * 0.55;
                        let cx = (page_w.0 - text_w_mm) / 2.0;
                        layer.use_text(payment.as_str(), font_size, Mm(cx), Mm(6.0), font);
                    }
                }
            }
            PdfBlock::ItineraryPage { img } => {
                let img_data = ::image::open(img)?;
                let (img_w, img_h) = img_data.dimensions();

                let margin = 6.0;
                let max_w = page_w.0 - margin - margin;
                let max_h = page_h.0 - margin - margin;

                let dpi_factor = 72.0 / 25.4;
                let scale = ((max_w * dpi_factor) / img_w.max(1) as f32)
                    .min((max_h * dpi_factor) / img_h.max(1) as f32);

                let final_w = img_w as f32 * scale * (25.4 / 72.0);
                let final_h = img_h as f32 * scale * (25.4 / 72.0);
                let x = (page_w.0 - final_w) / 2.0;
                let y = (page_h.0 - final_h) / 2.0;

                let rgba = img_data.to_rgba8();
                let (w, h) = rgba.dimensions();
                let mut rgb_data = Vec::with_capacity((w * h * 3) as usize);
                for y in 0..h {
                    for x in 0..w {
                        let px = rgba.get_pixel(x, y);
                        rgb_data.push(px[0]);
                        rgb_data.push(px[1]);
                        rgb_data.push(px[2]);
                    }
                }
                let image_xobj = ImageXObject {
                    width: Px(w as usize),
                    height: Px(h as usize),
                    color_space: ColorSpace::Rgb,
                    bits_per_component: ColorBits::Bit8,
                    interpolate: true,
                    image_data: rgb_data,
                    image_filter: None,
                    smask: None,
                    clipping_bbox: None,
                };
                let image = Image::from(image_xobj);
                image.add_to_layer(
                    layer.clone(),
                    ImageTransform {
                        translate_x: Some(Mm(x)),
                        translate_y: Some(Mm(y)),
                        scale_x: Some(final_w / (w as f32 * (25.4 / 72.0))),
                        scale_y: Some(final_h / (h as f32 * (25.4 / 72.0))),
                        rotate: None,
                        dpi: Some(72.0),
                    },
                );
            }
            PdfBlock::ItineraryTable { rows } => {
                use printpdf::path::*;
                use printpdf::Line;
                use printpdf::Point;

                let col_w = [Mm(45.0), Mm(65.0), Mm(110.0)];
                let total_w = col_w[0] + col_w[1] + col_w[2];
                let table_left = Mm((page_w.0 - total_w.0) / 2.0);
                let row_h = Mm(10.0);
                let header_top = page_h - Mm(25.0);
                let header_bot = header_top - row_h;

                let black = Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None));
                layer.set_outline_color(black);
                layer.set_outline_thickness(0.5);

                // Draw header rectangle
                let h_points = vec![
                    (Point::new(table_left, header_top), false),
                    (Point::new(table_left + total_w, header_top), false),
                    (Point::new(table_left + total_w, header_bot), false),
                    (Point::new(table_left, header_bot), false),
                ];
                let h_poly = Polygon {
                    rings: vec![h_points],
                    mode: PaintMode::Stroke,
                    winding_order: WindingOrder::NonZero,
                };
                layer.add_polygon(h_poly);

                let v1_x = table_left + col_w[0];
                let v2_x = table_left + col_w[0] + col_w[1];
                for vx in [v1_x, v2_x] {
                    let line = Line::from_iter(vec![
                        (Point::new(vx, header_top), false),
                        (Point::new(vx, header_bot), false),
                    ]);
                    layer.add_line(line);
                }

                let h_margin = Mm(5.0);
                let header_text_y = header_bot + Mm(2.5);
                layer.use_text("\u{884c}\u{7a0b}\u{5e8f}\u{53f7}", 12.0, table_left + h_margin, header_text_y, font);
                layer.use_text("\u{884c}\u{7a0b}\u{91d1}\u{989d}", 12.0, v1_x + h_margin, header_text_y, font);
                layer.use_text("\u{652f}\u{4ed8}\u{5355}\u{53f7}", 12.0, v2_x + h_margin, header_text_y, font);

                // Data rows
                for (i, (seq, amt, pay_id)) in rows.iter().enumerate() {
                    let row_top = header_bot - row_h * i as f32;
                    let row_bot = row_top - row_h;

                    // Cell rectangle
                    let r_points = vec![
                        (Point::new(table_left, row_top), false),
                        (Point::new(table_left + total_w, row_top), false),
                        (Point::new(table_left + total_w, row_bot), false),
                        (Point::new(table_left, row_bot), false),
                    ];
                    let r_poly = Polygon {
                        rings: vec![r_points],
                        mode: PaintMode::Stroke,
                        winding_order: WindingOrder::NonZero,
                    };
                    layer.add_polygon(r_poly);

                    // Vertical lines
                    for vx in [v1_x, v2_x] {
                        let line = Line::from_iter(vec![
                            (Point::new(vx, row_top), false),
                            (Point::new(vx, row_bot), false),
                        ]);
                        layer.add_line(line);
                    }

                    let cell_y = row_bot + Mm(2.5);
                    layer.use_text(&seq.to_string(), 11.0, table_left + h_margin, cell_y, font);
                    layer.use_text(&format!("{:.2}", amt), 11.0, v1_x + h_margin, cell_y, font);
                    layer.use_text(pay_id.as_str(), 11.0, v2_x + h_margin, cell_y, font);
                }
            }
        }
        Ok(())
    }

    let first_layer = doc.get_page(page1_idx).get_layer(layer1_idx);
    if let Some(first) = blocks.first() {
        render_block_body(&doc, &first_layer, first, &font, page_w, page_h)?;
    }
    for (i, block) in blocks.iter().enumerate().skip(1) {
        let (page_idx, layer_idx) = doc.add_page(page_w, page_h, format!("Layer {}", i + 1));
        let layer = doc.get_page(page_idx).get_layer(layer_idx);
        render_block_body(&doc, &layer, block, &font, page_w, page_h)?;
    }

    let file = fs::File::create(output)?;
    doc.save(&mut std::io::BufWriter::new(file))?;

    fs::remove_dir_all(&tmp_dir).ok();

    Ok(())
}
