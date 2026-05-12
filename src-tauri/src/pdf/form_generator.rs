use crate::models::reimbursement::ReimbursementForm;
use crate::models::invoice::InvoiceCategory;
use crate::models::match_result::MatchResult;
use genpdf::{Document, elements, fonts};
use std::error::Error;

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

pub fn generate_reimbursement_pdf(
    form: &ReimbursementForm,
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let font_family = load_chinese_fonts()?;
    let mut doc = Document::new(font_family);
    doc.set_title("差旅费报销表");

    doc.push(elements::Paragraph::new("差旅费报销表").aligned(genpdf::Alignment::Center));

    doc.push(elements::Paragraph::new(format!(
        "姓名：{}  部职别：{}", form.name, form.department
    )));
    doc.push(elements::Paragraph::new(format!(
        "出差日期：{} 至 {}  同行人数：{}", form.travel_start, form.travel_end, form.companions
    )));

    doc.push(elements::Paragraph::new("城市间交通费"));
    for s in &form.summaries {
        if matches!(s.category, InvoiceCategory::Train | InvoiceCategory::Flight | InvoiceCategory::TicketChange) {
            doc.push(elements::Paragraph::new(format!(
                "  {}  单据张数：{}  申报金额：{:.2}", category_label(&s.category), s.count, s.total_amount
            )));
        }
    }

    doc.push(elements::Paragraph::new("市内交通费"));
    if let Some(s) = form.summaries.iter().find(|s| matches!(s.category, InvoiceCategory::CityTransport)) {
        doc.push(elements::Paragraph::new(format!(
            "  单据张数：{}  申报金额：{:.2}", s.count, s.total_amount
        )));
    }

    doc.push(elements::Paragraph::new("住宿费"));
    if let Some(s) = form.summaries.iter().find(|s| matches!(s.category, InvoiceCategory::Hotel)) {
        doc.push(elements::Paragraph::new(format!(
            "  单据张数：{}  申报金额：{:.2}", s.count, s.total_amount
        )));
    }

    doc.push(elements::Paragraph::new("餐补/伙食补助"));
    if let Some(s) = form.summaries.iter().find(|s| matches!(s.category, InvoiceCategory::Meal)) {
        doc.push(elements::Paragraph::new(format!(
            "  申报金额：{:.2}", s.total_amount
        )));
    }

    if let Some(s) = form.summaries.iter().find(|s| matches!(s.category, InvoiceCategory::Other)) {
        doc.push(elements::Paragraph::new("其他"));
        doc.push(elements::Paragraph::new(format!(
            "  单据张数：{}  申报金额：{:.2}", s.count, s.total_amount
        )));
    }

    doc.push(elements::Paragraph::new(format!(
        "合计：{:.2} 元", form.total_amount
    )));

    doc.push(elements::Paragraph::new("\n出差人签字：          部门领导签字：          日期："));

    doc.render_to_file(output_path)?;
    Ok(())
}

/// 生成发票明细表格 PDF
pub fn generate_detail_table_pdf(
    match_results: &[MatchResult],
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let font_family = load_chinese_fonts()?;
    let mut doc = Document::new(font_family);
    doc.set_title("发票明细表");

    // 标题
    doc.push(elements::Paragraph::new("发票明细表").aligned(genpdf::Alignment::Center));
    doc.push(elements::Paragraph::new(""));

    // 创建表格：序号、发票号码、发票类别、发票备注、发票金额、支付渠道、支付单号、支付金额、优惠金额
    let mut table = elements::TableLayout::new(vec![1, 3, 2, 2, 2, 2, 3, 2, 2]);
    table.set_cell_decorator(elements::FrameCellDecorator::new(true, true, false));

    // 表头
    let mut header = table.row();
    header.push_element(elements::Paragraph::new("序号"));
    header.push_element(elements::Paragraph::new("发票号码"));
    header.push_element(elements::Paragraph::new("发票类别"));
    header.push_element(elements::Paragraph::new("发票备注"));
    header.push_element(elements::Paragraph::new("发票金额"));
    header.push_element(elements::Paragraph::new("支付渠道"));
    header.push_element(elements::Paragraph::new("支付单号"));
    header.push_element(elements::Paragraph::new("支付金额"));
    header.push_element(elements::Paragraph::new("优惠金额"));
    header.push()?;

    let mut row_index = 1;

    for result in match_results {
        let invoice = &result.invoice;
        let is_city_transport = matches!(invoice.category, InvoiceCategory::CityTransport);
        let has_itineraries = is_city_transport && !invoice.itineraries.is_empty();

        if has_itineraries {
            // 网约车发票：每条行程一行，共享发票信息
            for (i, itinerary) in invoice.itineraries.iter().enumerate() {
                let mut row = table.row();

                // 序号
                row.push_element(elements::Paragraph::new(format!("{}", row_index)));
                row_index += 1;

                // 发票号码（只在第一行显示）
                if i == 0 {
                    row.push_element(elements::Paragraph::new(invoice.invoice_number.clone()));
                } else {
                    row.push_element(elements::Paragraph::new(""));
                }

                // 发票类别
                row.push_element(elements::Paragraph::new(category_label(&invoice.category)));

                // 发票备注（行程信息）
                row.push_element(elements::Paragraph::new(format!(
                    "{} {} -> {}", itinerary.date_time, itinerary.pickup, itinerary.dropoff
                )));

                // 发票金额（只在第一行显示）
                if i == 0 {
                    row.push_element(elements::Paragraph::new(format!("{:.2}", invoice.amount)));
                } else {
                    row.push_element(elements::Paragraph::new(""));
                }

                // 支付渠道（只在第一行显示）
                if i == 0 {
                    let payment_source = result.payments.first()
                        .map(|p| format!("{:?}", p.source))
                        .unwrap_or_default();
                    row.push_element(elements::Paragraph::new(payment_source));
                } else {
                    row.push_element(elements::Paragraph::new(""));
                }

                // 支付单号（只在第一行显示）
                if i == 0 {
                    let payment_ids = result.payment_ids.join(", ");
                    row.push_element(elements::Paragraph::new(payment_ids));
                } else {
                    row.push_element(elements::Paragraph::new(""));
                }

                // 支付金额（行程金额）
                row.push_element(elements::Paragraph::new(format!("{:.2}", itinerary.amount)));

                // 优惠金额（发票金额与支付金额的差额）
                if i == 0 {
                    let total_payment: f64 = result.payments.iter().map(|p| p.amount).sum();
                    let discount = invoice.amount - total_payment;
                    if discount.abs() > 0.01 {
                        row.push_element(elements::Paragraph::new(format!("{:.2}", discount)));
                    } else {
                        row.push_element(elements::Paragraph::new("0.00"));
                    }
                } else {
                    row.push_element(elements::Paragraph::new(""));
                }

                row.push()?;
            }
        } else {
            // 普通发票：一行
            let mut row = table.row();

            // 序号
            row.push_element(elements::Paragraph::new(format!("{}", row_index)));
            row_index += 1;

            // 发票号码
            row.push_element(elements::Paragraph::new(invoice.invoice_number.clone()));

            // 发票类别
            row.push_element(elements::Paragraph::new(category_label(&invoice.category)));

            // 发票备注
            row.push_element(elements::Paragraph::new(invoice.seller_name.clone()));

            // 发票金额
            row.push_element(elements::Paragraph::new(format!("{:.2}", invoice.amount)));

            // 支付渠道
            let payment_source = result.payments.first()
                .map(|p| format!("{:?}", p.source))
                .unwrap_or_default();
            row.push_element(elements::Paragraph::new(payment_source));

            // 支付单号
            let payment_ids = result.payment_ids.join(", ");
            row.push_element(elements::Paragraph::new(payment_ids));

            // 支付金额
            let total_payment: f64 = result.payments.iter().map(|p| p.amount).sum();
            row.push_element(elements::Paragraph::new(format!("{:.2}", total_payment)));

            // 优惠金额
            let discount = invoice.amount - total_payment;
            if discount.abs() > 0.01 {
                row.push_element(elements::Paragraph::new(format!("{:.2}", discount)));
            } else {
                row.push_element(elements::Paragraph::new("0.00"));
            }

            row.push()?;
        }
    }

    doc.push(table);

    // 合计
    let total_invoice: f64 = match_results.iter().map(|r| r.invoice.amount).sum();
    let total_payment: f64 = match_results.iter()
        .map(|r| r.payments.iter().map(|p| p.amount).sum::<f64>())
        .sum();
    let total_discount = total_invoice - total_payment;

    doc.push(elements::Paragraph::new(""));
    doc.push(elements::Paragraph::new(format!(
        "合计：发票金额 {:.2} 元，支付金额 {:.2} 元，优惠金额 {:.2} 元",
        total_invoice, total_payment, total_discount
    )));

    doc.render_to_file(output_path)?;
    Ok(())
}
