use crate::models::reimbursement::ReimbursementForm;
use crate::models::invoice::InvoiceCategory;
use genpdf::{Document, elements, fonts};
use std::error::Error;

// 中文类别标签
fn category_label(cat: &InvoiceCategory) -> &str {
    match cat {
        InvoiceCategory::Train => "车、船票",
        InvoiceCategory::Flight => "飞机票",
        InvoiceCategory::TicketChange => "订（退、改签）票及交通保险费",
        InvoiceCategory::CityTransport => "市内交通费",
        InvoiceCategory::Hotel => "住宿费",
        InvoiceCategory::Meal => "餐补/伙食补助",
        InvoiceCategory::Other => "其他",
    }
}

pub fn generate_reimbursement_pdf(
    form: &ReimbursementForm,
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    // 加载中文字体
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
    doc.set_title("差旅费报销表");

    // 标题
    doc.push(elements::Paragraph::new("差旅费报销表").aligned(genpdf::Alignment::Center));

    // 基本信息
    doc.push(elements::Paragraph::new(format!(
        "姓名：{}  部职别：{}", form.name, form.department
    )));
    doc.push(elements::Paragraph::new(format!(
        "出差日期：{} 至 {}  同行人数：{}", form.travel_start, form.travel_end, form.companions
    )));

    // 城市间交通费
    doc.push(elements::Paragraph::new("城市间交通费"));
    for s in &form.summaries {
        if matches!(s.category, InvoiceCategory::Train | InvoiceCategory::Flight | InvoiceCategory::TicketChange) {
            doc.push(elements::Paragraph::new(format!(
                "  {}  单据张数：{}  申报金额：{:.2}", category_label(&s.category), s.count, s.total_amount
            )));
        }
    }

    // 市内交通费
    doc.push(elements::Paragraph::new("市内交通费"));
    if let Some(s) = form.summaries.iter().find(|s| matches!(s.category, InvoiceCategory::CityTransport)) {
        doc.push(elements::Paragraph::new(format!(
            "  单据张数：{}  申报金额：{:.2}", s.count, s.total_amount
        )));
    }

    // 住宿费
    doc.push(elements::Paragraph::new("住宿费"));
    if let Some(s) = form.summaries.iter().find(|s| matches!(s.category, InvoiceCategory::Hotel)) {
        doc.push(elements::Paragraph::new(format!(
            "  单据张数：{}  申报金额：{:.2}", s.count, s.total_amount
        )));
    }

    // 餐补
    doc.push(elements::Paragraph::new("餐补/伙食补助"));
    if let Some(s) = form.summaries.iter().find(|s| matches!(s.category, InvoiceCategory::Meal)) {
        doc.push(elements::Paragraph::new(format!(
            "  申报金额：{:.2}", s.total_amount
        )));
    }

    // 其他
    if let Some(s) = form.summaries.iter().find(|s| matches!(s.category, InvoiceCategory::Other)) {
        doc.push(elements::Paragraph::new("其他"));
        doc.push(elements::Paragraph::new(format!(
            "  单据张数：{}  申报金额：{:.2}", s.count, s.total_amount
        )));
    }

    // 总计
    doc.push(elements::Paragraph::new(format!(
        "合计：{:.2} 元", form.total_amount
    )));

    // 签名区
    doc.push(elements::Paragraph::new("\n出差人签字：          部门领导签字：          日期："));

    doc.render_to_file(output_path)?;
    Ok(())
}
