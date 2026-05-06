use crate::models::match_result::MatchResult;
use genpdf::{Document, elements, fonts};
use std::error::Error;

pub fn generate_comparison_pdf(
    match_results: &[MatchResult],
    unmatched_invoice_ids: &[String],
    unmatched_payment_ids: &[String],
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    // 加载中文字体（与 form_generator 共用字体路径）
    let font_dir = "/usr/share/fonts/truetype/noto-cjk/";
    let regular = fonts::FontData::load(
        format!("{}NotoSansSC-Regular.ttf", font_dir),
        None,
    )?;
    let bold = fonts::FontData::load(
        format!("{}NotoSansSC-Bold.ttf", font_dir),
        None,
    )?;
    let font_family = fonts::FontFamily {
        regular: regular.clone(),
        bold: bold.clone(),
        italic: regular,
        bold_italic: bold,
    };

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
