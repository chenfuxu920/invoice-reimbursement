use crate::models::match_result::MatchResult;
use genpdf::{Document, elements, fonts};
use std::error::Error;

fn load_chinese_fonts() -> Result<fonts::FontFamily<fonts::FontData>, Box<dyn Error>> {
    let font_candidates: Vec<(&str, &str)> = if cfg!(target_os = "windows") {
        vec![
            ("C:/Windows/Fonts/simhei.ttf", "C:/Windows/Fonts/simhei.ttf"),
            ("C:/Windows/Fonts/simfang.ttf", "C:/Windows/Fonts/simfang.ttf"),
            ("C:/Windows/Fonts/simkai.ttf", "C:/Windows/Fonts/simkai.ttf"),
        ]
    } else {
        vec![
            ("/usr/share/fonts/truetype/noto-cjk/NotoSansSC-Regular.ttf", "/usr/share/fonts/truetype/noto-cjk/NotoSansSC-Bold.ttf"),
            ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc"),
        ]
    };

    for (regular_path, bold_path) in font_candidates {
        if std::path::Path::new(regular_path).exists() {
            let regular = fonts::FontData::load(regular_path, None)?;
            let bold = if regular_path != bold_path && std::path::Path::new(bold_path).exists() {
                fonts::FontData::load(bold_path, None)?
            } else {
                regular.clone()
            };
            return Ok(fonts::FontFamily {
                regular: regular.clone(),
                bold: bold.clone(),
                italic: regular,
                bold_italic: bold,
            });
        }
    }
    Err("No Chinese font found".into())
}

pub fn generate_comparison_pdf(
    match_results: &[MatchResult],
    unmatched_invoice_ids: &[String],
    unmatched_payment_ids: &[String],
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let font_family = load_chinese_fonts()?;

    let mut doc = Document::new(font_family);
    doc.set_title("发票-支付对照表");

    // 标题
    doc.push(elements::Paragraph::new("发票-支付对照表").aligned(genpdf::Alignment::Center));

    // 已匹配项
    doc.push(elements::Paragraph::new("\n已匹配项目："));
    for (i, result) in match_results.iter().enumerate() {
        doc.push(elements::Paragraph::new(format!(
            "\n{}. 发票号码：{}  金额：{:.2}  类型：{:?}  差额：{:.2}",
            i + 1,
            result.invoice.invoice_number,
            result.invoice.amount,
            result.match_type,
            result.amount_diff,
        )));

        doc.push(elements::Paragraph::new("   对应支付："));
        for p in &result.payments {
            doc.push(elements::Paragraph::new(format!(
                "     - {}  金额：{:.2}  时间：{}",
                p.merchant_name, p.amount, p.transaction_time
            )));
        }
    }

    // 未匹配发票
    if !unmatched_invoice_ids.is_empty() {
        doc.push(elements::Paragraph::new("\n未匹配发票："));
        for id in unmatched_invoice_ids {
            doc.push(elements::Paragraph::new(format!("  - {}", id)));
        }
    }

    // 未匹配支付
    if !unmatched_payment_ids.is_empty() {
        doc.push(elements::Paragraph::new("\n未匹配支付："));
        for id in unmatched_payment_ids {
            doc.push(elements::Paragraph::new(format!("  - {}", id)));
        }
    }

    doc.render_to_file(output_path)?;
    Ok(())
}
