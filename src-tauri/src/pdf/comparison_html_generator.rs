use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use crate::models::invoice::{InvoiceCategory, InvoiceSource};
use crate::models::match_result::MatchResult;

enum OutputBlock {
    Invoice { img: String, payment: String },
    /// 手动添加的空发票：无源图片，留白页用于粘贴纸质票据
    BlankInvoice { payment: String },
    Itinerary {
        imgs: Vec<String>,
        rows: Vec<(usize, f64, String)>,
    },
}

pub fn generate_comparison_html(
    match_results: &[MatchResult],
    invoice_dir: &str,
    output_dir: &str,
    dpi: u32,
) -> Result<String, Box<dyn Error>> {
    std::fs::create_dir_all(output_dir)?;

    let mut seen_pdfs: HashMap<String, &MatchResult> = HashMap::new();
    for result in match_results {
        if let InvoiceSource::Pdf(pdf_path) = &result.invoice.source {
            seen_pdfs.entry(pdf_path.clone()).or_insert(result);
        }
    }

    let itinerary_pdfs = find_itinerary_pdfs(invoice_dir, &seen_pdfs);

    let mut blocks: Vec<OutputBlock> = Vec::new();
    let mut global_seq = 0usize;
    let mut itin_idx = 0usize;

    for result in match_results {
        match &result.invoice.source {
            InvoiceSource::Manual => {
                let payment_text = build_payment_text(result);
                blocks.push(OutputBlock::BlankInvoice { payment: payment_text });
            }
            InvoiceSource::Pdf(pdf_path) => {
                let img_path = super::image_embedder::render_pdf_page_to_png(
                    pdf_path, 0, output_dir, dpi,
                )?;
                let rel = img_path.file_name().unwrap_or_default().to_string_lossy().to_string();

                let payment_text = build_payment_text(result);
                blocks.push(OutputBlock::Invoice { img: rel, payment: payment_text });
            }
            _ => {}
        }

        if matches!(result.invoice.category, InvoiceCategory::CityTransport) {
            let mut itinerary_imgs: Vec<String> = Vec::new();
            if itin_idx < itinerary_pdfs.len() {
                let pdf = &itinerary_pdfs[itin_idx];
                let imgs = super::image_embedder::render_pdf_all_pages_to_pngs(pdf, output_dir, dpi)?;
                for p in imgs {
                    itinerary_imgs.push(p.file_name().unwrap_or_default().to_string_lossy().to_string());
                }
                itin_idx += 1;
            }

            let mut rows: Vec<(usize, f64, String)> = Vec::new();
            for (i, itin) in result.invoice.itineraries.iter().enumerate() {
                global_seq += 1;
                let pay_id = result.payment_for_itinerary(i)
                    .map(|p| p.transaction_id.clone())
                    .unwrap_or_default();
                rows.push((global_seq, itin.amount, pay_id));
            }

            blocks.push(OutputBlock::Itinerary { imgs: itinerary_imgs, rows });
        }
    }

    let html_path = Path::new(output_dir).join("发票对照单.html");
    let html = build_html(&blocks);
    std::fs::write(&html_path, &html)?;
    Ok(html_path.to_string_lossy().to_string())
}

fn normalize_path(s: &str) -> String {
    s.replace('/', "\\")
}

/// 构造支付单号文本。市内交通且有行程明细时返回空（由行程表格单独展示）。
fn build_payment_text(result: &MatchResult) -> String {
    let has_itinerary = matches!(result.invoice.category, InvoiceCategory::CityTransport)
        && !result.invoice.itineraries.is_empty();
    if has_itinerary || result.payments.is_empty() {
        return String::new();
    }
    if result.payments.len() == 1 {
        format!("支付单号：{}", result.payments[0].transaction_id)
    } else {
        let ids: Vec<&str> = result.payments.iter().map(|p| p.transaction_id.as_str()).collect();
        format!("支付单号：{}", ids.join("，"))
    }
}

fn find_itinerary_pdfs(
    dir: &str,
    invoice_pdfs: &HashMap<String, &MatchResult>,
) -> Vec<String> {
    let dir_path = Path::new(dir);
    if !dir_path.is_dir() {
        return Vec::new();
    }

    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext == "pdf" {
                let path_str = path.to_string_lossy().to_string();
                let normalized = normalize_path(&path_str);
                if !invoice_pdfs.contains_key(&normalized) {
                    result.push(path_str);
                }
            }
        }
    }
    result.sort();
    result
}

fn build_html(blocks: &[OutputBlock]) -> String {
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"UTF-8\">\n<title>发票对照单</title>\n<style>");
    html.push_str("\n  @page { size: A4 landscape; margin: 12mm; }");
    html.push_str("\n  * { margin: 0; padding: 0; box-sizing: border-box; }");
    html.push_str("\n  body { font-family: \"SimSun\", serif; background: #f0f0f0; }");
    html.push_str("\n  .page { page-break-after: always; width: 771px; min-height: 516px; display: flex; flex-direction: column; align-items: center; justify-content: center; background: #fff; margin: 0 auto; padding: 0; }");
    html.push_str("\n  .invoice-img { max-width: 95%; max-height: 80%; object-fit: contain; }");
    html.push_str("\n  .payment-bar { width: 100%; text-align: center; padding: 10px 0; font-size: 16px; font-weight: bold; border-top: 2px dashed #333; margin-top: 8px; font-family: \"Consolas\", monospace; }");
    html.push_str("\n  .itinerary-img { max-width: 92%; max-height: 88%; object-fit: contain; }");
    html.push_str("\n  .table-page { justify-content: flex-start; padding-top: 30px; }");
    html.push_str("\n  .table-page h3 { font-size: 16px; margin-bottom: 15px; letter-spacing: 2px; }");
    html.push_str("\n  .pay-table { width: 85%; border-collapse: collapse; font-size: 14px; margin-top: 10px; }");
    html.push_str("\n  .pay-table th { border: 1px solid #000; padding: 8px 10px; background: #e8e8e8; font-weight: bold; text-align: center; }");
    html.push_str("\n  .pay-table td { border: 1px solid #000; padding: 6px 10px; text-align: center; }");
    html.push_str("\n  .col-seq { width: 15%; } .col-amt { width: 25%; } .col-pay { width: 60%; font-family: \"Consolas\", monospace; }");
    html.push_str("\n  .blank-page { justify-content: space-between; padding: 12mm; }");
    html.push_str("\n  .paste-placeholder { flex: 1; display: flex; align-items: center; justify-content: center; border: 2px dashed #999; color: #999; font-size: 18px; letter-spacing: 2px; margin-bottom: 8px; }");
    html.push_str("\n  @media print { body { background: #fff; } .page { margin: 0; } }");
    html.push_str("\n</style>\n</head>\n<body>\n");

    for block in blocks {
        match block {
            OutputBlock::Invoice { img, payment } => {
                let show_payment = !payment.is_empty();
                html.push_str("<div class=\"page\">\n  <div style=\"position: relative; display: flex; flex-direction: column; align-items: center;\">\n    <img class=\"invoice-img\" src=\"");
                html.push_str(img);
                html.push_str("\" alt=\"发票\">");
                if show_payment {
                    html.push_str("\n    <div style=\"position: absolute; bottom: 20px; left: 0; right: 0; text-align: center; font-size: 15px; font-weight: bold; font-family: \"Consolas\", monospace; color: #000;\">");
                    html.push_str(payment);
                    html.push_str("</div>");
                }
                html.push_str("\n  </div>\n</div>\n");
            }
            OutputBlock::BlankInvoice { payment } => {
                let show_payment = !payment.is_empty();
                html.push_str("<div class=\"page blank-page\">\n  <div class=\"paste-placeholder\">（此处粘贴纸质票据）</div>\n");
                if show_payment {
                    html.push_str("  <div class=\"payment-bar\">");
                    html.push_str(&payment);
                    html.push_str("</div>\n");
                }
                html.push_str("</div>\n");
            }
            OutputBlock::Itinerary { imgs, rows } => {
                for img_rel in imgs {
                    html.push_str("<div class=\"page\">\n  <img class=\"itinerary-img\" src=\"");
                    html.push_str(img_rel);
                    html.push_str("\" alt=\"行程单\">\n</div>\n");
                }

                if !rows.is_empty() {
                    let max_rows_per_page: usize = 17;
                    let total: f64 = rows.iter().map(|(_, amt, _)| amt).sum();
                    let chunks: Vec<_> = rows.chunks(max_rows_per_page).collect();
                    let chunk_count = chunks.len();
                    for (i, chunk) in chunks.into_iter().enumerate() {
                        let is_last = i == chunk_count - 1;
                        html.push_str("<div class=\"page table-page\">\n  <h3>行程支付明细</h3>\n  <table class=\"pay-table\">\n    <thead>\n      <tr><th class=\"col-seq\">行程序号</th><th class=\"col-amt\">行程金额</th><th class=\"col-pay\">支付单号</th></tr>\n    </thead>\n    <tbody>\n");
                        for (seq, amt, pay_id) in chunk {
                            html.push_str(&format!("      <tr><td>{}</td><td>{:.2}</td><td>{}</td></tr>\n", seq, amt, pay_id));
                        }
                        if is_last {
                            html.push_str(&format!("      <tr style=\"font-weight: bold;\"><td>合计</td><td>{:.2}</td><td></td></tr>\n", total));
                        }
                        html.push_str("    </tbody>\n  </table>\n</div>\n");
                    }
                }
            }
        }
    }

    html.push_str("</body>\n</html>\n");
    html
}
