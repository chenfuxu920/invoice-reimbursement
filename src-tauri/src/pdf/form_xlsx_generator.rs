use crate::models::invoice::InvoiceCategory;
use crate::models::match_result::MatchResult;
use crate::models::payment::PaymentSource;
use crate::models::reimbursement::ReimbursementForm;
use rust_xlsxwriter::*;
use std::error::Error;

/// Generates an Excel (.xlsx) file containing two sheets:
///   1. "差旅费报销单" — the main reimbursement form
///   2. "发票明细"     — invoice detail table
pub fn generate_reimbursement_xlsx(
    form: &ReimbursementForm,
    match_results: &[MatchResult],
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let mut workbook = Workbook::new();

    // ── Sheet 1: 差旅费报销单 ──
    let sheet1 = workbook.add_worksheet();
    sheet1.set_name("差旅费报销单")?;
    build_reimbursement_sheet(sheet1, form)?;

    // ── Sheet 2: 发票明细 ──
    let sheet2 = workbook.add_worksheet();
    sheet2.set_name("发票明细")?;
    build_invoice_detail_sheet(sheet2, match_results)?;

    workbook.save(output_path)?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Sheet 1 — 差旅费报销单
// ═══════════════════════════════════════════════════════════════════

fn build_reimbursement_sheet(
    ws: &mut Worksheet,
    form: &ReimbursementForm,
) -> Result<(), Box<dyn Error>> {
    // ── Column widths (12 cols: A–L) ──
    ws.set_column_width(0, 8.0)?; // A  — rowspan / category label
    ws.set_column_width(1, 12.0)?; // B  — label / count / level
    ws.set_column_width(2, 10.0)?; // C  — 单据张数 / 人数
    ws.set_column_width(3, 12.0)?; // D  — 申报金额
    ws.set_column_width(4, 10.0)?; // E  — 核准金额
    ws.set_column_width(5, 3.0)?; // F  — spacer column
    ws.set_column_width(6, 8.0)?; // G  — hotel rowspan / category label
    ws.set_column_width(7, 6.0)?; // H  — 人数
    ws.set_column_width(8, 6.0)?; // I  — 天数
    ws.set_column_width(9, 8.0)?; // J  — 标准
    ws.set_column_width(10, 12.0)?; // K — 申报金额
    ws.set_column_width(11, 10.0)?; // L — 核准金额

    // ── Reusable formats ──
    let thin = Format::new().set_border(FormatBorder::Thin);

    let title_fmt = Format::new()
        .set_bold()
        .set_font_size(16)
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    let header_fmt = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    let label_fmt = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    let cell_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_text_wrap()
        .set_align(FormatAlign::VerticalCenter);

    let cell_center_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    let _cell_left_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::VerticalCenter);

    let amt_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_num_format("0.00")
        .set_align(FormatAlign::VerticalCenter);

    let amt_center_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_num_format("0.00")
        .set_align(FormatAlign::Center);

    let sec_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_bold()
        .set_text_wrap()
        .set_align(FormatAlign::Center)
        .set_rotation(90);

    let sum_fmt = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    // ════════════════════════════════════════════
    //  Row 0 — Title (merged A1:L1)
    // ════════════════════════════════════════════
    ws.merge_range(0, 0, 0, 11, "差旅费报销单", &title_fmt)?;
    ws.set_row_height(0, 40)?;

    // ════════════════════════════════════════════
    //  Row 1 — 姓名 / 部职别 / 同行人数
    // ════════════════════════════════════════════
    ws.write_string_with_format(1, 0, "姓名", &label_fmt)?;
    ws.write_string_with_format(1, 1, &form.name, &cell_fmt)?;

    ws.write_string_with_format(1, 2, "部职别", &label_fmt)?;
    ws.merge_range(1, 3, 1, 4, &form.department, &cell_fmt)?;

    ws.write_string_with_format(1, 5, "同行人数", &label_fmt)?;
    let companions_str = if form.companions > 0 {
        form.companions.to_string()
    } else {
        String::new()
    };
    ws.merge_range(1, 6, 1, 11, &companions_str, &cell_center_fmt)?;

    // ════════════════════════════════════════════
    //  Row 2 — 到达地点 / 出差日期
    // ════════════════════════════════════════════
    ws.write_string_with_format(2, 0, "到达地点", &label_fmt)?;
    ws.merge_range(2, 1, 2, 2, &form.destination, &cell_fmt)?;

    ws.write_string_with_format(2, 3, "出差日期", &label_fmt)?;
    ws.write_string_with_format(2, 4, &form.travel_start, &cell_center_fmt)?;
    ws.write_string_with_format(2, 5, "至", &cell_center_fmt)?;
    ws.write_string_with_format(2, 6, &form.travel_end, &cell_center_fmt)?;

    ws.write_string_with_format(2, 7, &form.travel_days.to_string(), &cell_center_fmt)?;
    ws.write_string_with_format(2, 8, "天", &cell_center_fmt)?;
    ws.merge_range(2, 9, 2, 11, "", &cell_center_fmt)?;

    // ════════════════════════════════════════════
    //  Row 3 — Header row (12 cols: A–L)
    // ════════════════════════════════════════════
    // Left: A-B:类别 | C:单据张数 | D:申报金额 | E:核准金额
    ws.merge_range(3, 0, 3, 1, "类  别", &header_fmt)?;
    ws.write_string_with_format(3, 2, "单据张数", &header_fmt)?;
    ws.write_string_with_format(3, 3, "申报金额", &header_fmt)?;
    ws.write_string_with_format(3, 4, "核准金额", &header_fmt)?;

    // Right: F-G:类别 | H:人数 | I:天数 | J:标准 | K:申报金额 | L:核准金额
    ws.merge_range(3, 5, 3, 6, "类  别", &header_fmt)?;
    ws.write_string_with_format(3, 7, "人数", &header_fmt)?;
    ws.write_string_with_format(3, 8, "天数", &header_fmt)?;
    ws.write_string_with_format(3, 9, "标准", &header_fmt)?;
    ws.write_string_with_format(3, 10, "申报金额", &header_fmt)?;
    ws.write_string_with_format(3, 11, "核准金额", &header_fmt)?;

    // ════════════════════════════════════════════
    //  Rows 4–8 — Transport (left) + Hotel (right)
    // ════════════════════════════════════════════
    write_transport_hotel_section(ws, form, &cell_center_fmt, &cell_fmt, &amt_fmt, &sec_fmt)?;

    // ════════════════════════════════════════════
    //  Row 9 — 行李托运费 (left) / 伙食补助 (right)
    // ════════════════════════════════════════════
    ws.merge_range(9, 0, 9, 1, "行李托运费", &label_fmt)?;
    ws.write_blank(9, 2, &thin)?;
    ws.write_blank(9, 3, &thin)?;
    ws.write_blank(9, 4, &thin)?;

    ws.merge_range(9, 5, 9, 6, "计发伙食补助费", &label_fmt)?;
    ws.write_number_with_format(9, 7, form.meal_subsidy.persons as f64, &amt_center_fmt)?;
    ws.write_number_with_format(9, 8, form.meal_subsidy.days as f64, &amt_center_fmt)?;
    ws.write_number_with_format(9, 9, form.meal_subsidy.daily_rate, &amt_fmt)?;
    ws.write_number_with_format(9, 10, form.meal_subsidy.amount, &amt_fmt)?;
    ws.write_blank(9, 11, &thin)?;

    // ════════════════════════════════════════════
    //  Row 10 — 凭据报销伙食费 (left) / 预借款 (right)
    // ════════════════════════════════════════════
    ws.merge_range(10, 0, 10, 1, "凭据报销伙食费", &label_fmt)?;
    ws.write_blank(10, 2, &thin)?;
    ws.write_blank(10, 3, &thin)?;
    ws.write_blank(10, 4, &thin)?;

    ws.merge_range(10, 5, 10, 6, "预  借  款", &label_fmt)?;
    ws.merge_range(
        10,
        7,
        10,
        11,
        &format!("¥: {:.2}", form.advance_payment),
        &cell_center_fmt,
    )?;

    // ════════════════════════════════════════════
    //  Row 11 — 申报金额
    // ════════════════════════════════════════════
    ws.merge_range(11, 0, 11, 1, "申报金额", &label_fmt)?;
    ws.merge_range(
        11,
        2,
        11,
        11,
        &format!("(¥: {:.2})", form.total_amount),
        &sum_fmt,
    )?;
    ws.set_row_height(11, 30)?;

    // ════════════════════════════════════════════
    //  Row 12 — 核准金额 (with Chinese uppercase)
    // ════════════════════════════════════════════
    let total_chinese = amount_to_chinese(form.total_amount);
    ws.merge_range(12, 0, 12, 1, "核准金额", &label_fmt)?;
    ws.merge_range(12, 2, 12, 11, &format!("{} (¥: )", total_chinese), &sum_fmt)?;
    ws.set_row_height(12, 30)?;

    // ════════════════════════════════════════════
    //  Row 13 — (blank separator)
    // ════════════════════════════════════════════
    // Leave row 13 empty as a visual gap.

    // ════════════════════════════════════════════
    //  Row 14 — Signature line
    // ════════════════════════════════════════════
    let sig_fmt = Format::new()
        .set_font_size(11)
        .set_align(FormatAlign::VerticalCenter);
    ws.merge_range(
        14,
        0,
        14,
        11,
        "出差人签字:________    部门领导签字:________    日期:________",
        &sig_fmt,
    )?;
    ws.set_row_height(14, 28)?;

    Ok(())
}

/// Writes the transport (left, cols A–E) and hotel (right, cols G–L) section rows.
///
/// The left side has 3 transport detail rows + 1 subtotal row (= 4 rows) plus an
/// optional overflow row for "市内交通费".  The right side has 4 hotel level rows
/// + 1 subtotal row (= 5 rows).  The section uses the larger count (5 rows total).
///
/// Row layout (0-indexed within this section, offset by 4 in the sheet):
///   idx 0 — 车船票 (left) / 战区级以上 (right)
///   idx 1 — 飞机票 (left) / 军级 (right)
///   idx 2 — 订（退、改签）票 (left) / 师级 (right)
///   idx 3 — 小计 (left) / 其他人员 (right)
///   idx 4 — 市内交通费 (left) / 小计 (right)
fn write_transport_hotel_section(
    ws: &mut Worksheet,
    form: &ReimbursementForm,
    center_fmt: &Format,
    _cell_fmt: &Format,
    amt_fmt: &Format,
    sec_fmt: &Format,
) -> Result<(), Box<dyn Error>> {
    let transport_labels = ["车、船票", "飞机票", "订（退、改签）票"];
    let hotel_level_names = ["战区级以上", "军级", "师级", "其他人员"];

    // Map form transport_details into fixed slots
    let mut details = [None, None, None];
    for d in &form.transport_details {
        for (i, lbl) in transport_labels.iter().enumerate() {
            if d.label == *lbl {
                details[i] = Some(d);
            }
        }
    }

    let transport_row_count: usize = 4; // 3 detail + 1 subtotal
    let hotel_row_count: usize = hotel_level_names.len() + 1; // 5
    let total_rows = transport_row_count.max(hotel_row_count); // 5

    let thin = Format::new().set_border(FormatBorder::Thin);

    // Rowspan merges
    ws.merge_range(4, 0, 7, 0, "城市间\n交通费", sec_fmt)?;
    ws.merge_range(4, 5, 8, 5, "住\n宿\n费", sec_fmt)?;

    for row_idx in 0..total_rows {
        let xlsx_row = 4 + row_idx as u32;

        // ── Left side: cols A(0) already merged via rowspan; cols B–E ──
        if row_idx < 3 {
            // Transport detail row
            let label = transport_labels[row_idx];
            if let Some(detail) = &details[row_idx] {
                ws.write_string_with_format(xlsx_row, 1, label, center_fmt)?;
                ws.write_string_with_format(xlsx_row, 2, &detail.count.to_string(), center_fmt)?;
                ws.write_number_with_format(xlsx_row, 3, detail.amount, amt_fmt)?;
                ws.write_blank(xlsx_row, 4, &thin)?;
            } else {
                ws.write_string_with_format(xlsx_row, 1, label, center_fmt)?;
                ws.write_blank(xlsx_row, 2, &thin)?;
                ws.write_blank(xlsx_row, 3, &thin)?;
                ws.write_blank(xlsx_row, 4, &thin)?;
            }
        } else if row_idx == 3 {
            // Transport subtotal
            ws.write_string_with_format(xlsx_row, 1, "小  计", center_fmt)?;
            ws.write_blank(xlsx_row, 2, &thin)?;
            ws.write_number_with_format(xlsx_row, 3, form.transport_subtotal, amt_fmt)?;
            ws.write_blank(xlsx_row, 4, &thin)?;
        } else {
            // row_idx == 4: 市内交通费
            ws.merge_range(xlsx_row, 1, xlsx_row, 2, "市内交通费", center_fmt)?;
            ws.write_string_with_format(
                xlsx_row,
                3,
                &format!("{:.2}", form.city_transport_actual_amount),
                amt_fmt,
            )?;
            ws.write_number_with_format(xlsx_row, 4, form.city_transport_amount, amt_fmt)?;
        }

        // ── Hotel right side: col F(5) is already merged rowspan; cols G(6)–L(11) ──
        if row_idx < 4 {
            let level_name = hotel_level_names[row_idx];
            let detail = form.hotel_levels.iter().find(|h| h.level == level_name);
            if let Some(d) = detail {
                ws.write_string_with_format(xlsx_row, 6, level_name, center_fmt)?;
                ws.write_number_with_format(xlsx_row, 7, d.persons as f64, center_fmt)?;
                ws.write_number_with_format(xlsx_row, 8, d.days as f64, center_fmt)?;
                ws.write_number_with_format(xlsx_row, 9, d.daily_rate, amt_fmt)?;
                ws.write_number_with_format(xlsx_row, 10, d.actual_amount, amt_fmt)?;
                ws.write_number_with_format(xlsx_row, 11, d.amount, amt_fmt)?;
            } else {
                ws.write_string_with_format(xlsx_row, 6, level_name, center_fmt)?;
                ws.write_blank(xlsx_row, 7, &thin)?;
                ws.write_blank(xlsx_row, 8, &thin)?;
                ws.write_blank(xlsx_row, 9, &thin)?;
                ws.write_blank(xlsx_row, 10, &thin)?;
                ws.write_blank(xlsx_row, 11, &thin)?;
            }
        } else {
            // row_idx == 4: Hotel subtotal
            let hotel_actual_total: f64 = form.hotel_levels.iter().map(|h| h.actual_amount).sum();
            ws.write_string_with_format(xlsx_row, 6, "小  计", center_fmt)?;
            ws.write_blank(xlsx_row, 7, &thin)?;
            ws.write_blank(xlsx_row, 8, &thin)?;
            ws.write_blank(xlsx_row, 9, &thin)?;
            ws.write_number_with_format(xlsx_row, 10, hotel_actual_total, amt_fmt)?;
            ws.write_number_with_format(xlsx_row, 11, form.hotel_subtotal, amt_fmt)?;
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Sheet 2 — 发票明细
// ═══════════════════════════════════════════════════════════════════

fn build_invoice_detail_sheet(
    ws: &mut Worksheet,
    match_results: &[MatchResult],
) -> Result<(), Box<dyn Error>> {
    // ── Column widths (9 cols) ──
    ws.set_column_width(0, 6.0)?; // A — 序号
    ws.set_column_width(1, 16.0)?; // B — 发票号码
    ws.set_column_width(2, 14.0)?; // C — 发票类别
    ws.set_column_width(3, 30.0)?; // D — 发票备注
    ws.set_column_width(4, 12.0)?; // E — 发票金额
    ws.set_column_width(5, 10.0)?; // F — 支付渠道
    ws.set_column_width(6, 20.0)?; // G — 支付单号
    ws.set_column_width(7, 12.0)?; // H — 支付金额
    ws.set_column_width(8, 10.0)?; // I — 优惠金额

    // ── Formats ──
    let thin = Format::new().set_border(FormatBorder::Thin);
    let thin_center = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let thin_left = Format::new()
        .set_border(FormatBorder::Thin)
        .set_text_wrap()
        .set_align(FormatAlign::VerticalCenter);
    let title_fmt = Format::new()
        .set_bold()
        .set_font_size(14)
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let header_fmt = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let num_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_num_format("0.00")
        .set_align(FormatAlign::VerticalCenter);
    let total_label_fmt = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let total_num_fmt = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_num_format("0.00")
        .set_align(FormatAlign::VerticalCenter);

    // ════════════════════════════════════════════
    //  Row 0 — Title
    // ════════════════════════════════════════════
    ws.merge_range(0, 0, 0, 8, "发票明细表", &title_fmt)?;
    ws.set_row_height(0, 32)?;

    // ════════════════════════════════════════════
    //  Row 1 — Headers
    // ════════════════════════════════════════════
    let headers = [
        "序号",
        "发票号码",
        "发票类别",
        "发票备注",
        "发票金额",
        "支付渠道",
        "支付单号",
        "支付金额",
        "优惠金额",
    ];
    for (col, hdr) in headers.iter().enumerate() {
        ws.write_string_with_format(1, col as u16, *hdr, &header_fmt)?;
    }
    ws.set_row_height(1, 24)?;

    // ════════════════════════════════════════════
    //  Rows 2+ — Data
    // ════════════════════════════════════════════
    let mut row_index: u32 = 2; // Next free xlsx row
    let mut seq: u32 = 1;

    for result in match_results {
        let invoice = &result.invoice;
        let is_city_transport = matches!(invoice.category, InvoiceCategory::CityTransport);
        let has_itineraries = is_city_transport && !invoice.itineraries.is_empty();

        if has_itineraries {
            // Each itinerary gets its own row; invoice_number / amount only on first.
            for (i, itinerary) in invoice.itineraries.iter().enumerate() {
                let r = row_index;
                row_index += 1;

                // 序号
                ws.write_number_with_format(r, 0, seq as f64, &thin_center)?;
                seq += 1;

                // 发票号码 (first row only)
                if i == 0 {
                    ws.write_string_with_format(r, 1, &invoice.invoice_number, &thin_center)?;
                } else {
                    ws.write_blank(r, 1, &thin)?;
                }

                // 发票类别
                ws.write_string_with_format(r, 2, category_label(&invoice.category), &thin_center)?;

                // 发票备注 — itinerary detail
                ws.write_string_with_format(
                    r,
                    3,
                    &format!(
                        "{}  {} → {}",
                        itinerary.date_time, itinerary.pickup, itinerary.dropoff
                    ),
                    &thin_left,
                )?;

                // 发票金额 (first row only)
                if i == 0 {
                    ws.write_number_with_format(r, 4, invoice.amount, &num_fmt)?;
                } else {
                    ws.write_blank(r, 4, &thin)?;
                }

                // 支付渠道 (first row only)
                if i == 0 {
                    let source_label = result
                        .payment_for_itinerary(0)
                        .map(|p| payment_source_label(&p.source))
                        .unwrap_or("");
                    ws.write_string_with_format(r, 5, source_label, &thin_center)?;
                } else {
                    ws.write_blank(r, 5, &thin)?;
                }

                // 支付单号 (first row only)
                if i == 0 {
                    let payment_ids = result.payment_ids.join(", ");
                    ws.write_string_with_format(r, 6, &payment_ids, &thin_center)?;
                } else {
                    ws.write_blank(r, 6, &thin)?;
                }

                // 支付金额 — itinerary amount
                ws.write_number_with_format(r, 7, itinerary.amount, &num_fmt)?;

                // 优惠金额 (first row only — invoice - total_payment)
                if i == 0 {
                    let total_payment: f64 = result.payments.iter().map(|p| p.amount).sum();
                    let discount = invoice.amount - total_payment;
                    ws.write_number_with_format(r, 8, discount, &num_fmt)?;
                } else {
                    ws.write_blank(r, 8, &thin)?;
                }
            }
        } else {
            // One row per non-itinerary match result
            let r = row_index;
            row_index += 1;

            // 序号
            ws.write_number_with_format(r, 0, seq as f64, &thin_center)?;
            seq += 1;

            // 发票号码
            ws.write_string_with_format(r, 1, &invoice.invoice_number, &thin_center)?;

            // 发票类别
            ws.write_string_with_format(r, 2, category_label(&invoice.category), &thin_center)?;

            // 发票备注 — seller_name for hotels, remarks for others
            let remark = if matches!(invoice.category, InvoiceCategory::Hotel) {
                &invoice.seller_name
            } else {
                &invoice.remarks
            };
            ws.write_string_with_format(r, 3, remark, &thin_left)?;

            // 发票金额
            ws.write_number_with_format(r, 4, invoice.amount, &num_fmt)?;

            // 支付渠道
            let source_label = result
                .payments
                .first()
                .map(|p| payment_source_label(&p.source))
                .unwrap_or("");
            ws.write_string_with_format(r, 5, source_label, &thin_center)?;

            // 支付单号
            let payment_ids = result.payment_ids.join(", ");
            ws.write_string_with_format(r, 6, &payment_ids, &thin_center)?;

            // 支付金额
            let total_payment: f64 = result.payments.iter().map(|p| p.amount).sum();
            ws.write_number_with_format(r, 7, total_payment, &num_fmt)?;

            // 优惠金额
            let discount = invoice.amount - total_payment;
            ws.write_number_with_format(r, 8, discount, &num_fmt)?;
        }
    }

    // ════════════════════════════════════════════
    //  Footer — blank row then 合计
    // ════════════════════════════════════════════
    row_index += 1; // skip a blank row

    let total_invoice: f64 = match_results.iter().map(|r| r.invoice.amount).sum();
    let total_payment: f64 = match_results
        .iter()
        .map(|r| r.payments.iter().map(|p| p.amount).sum::<f64>())
        .sum();
    let total_discount = total_invoice - total_payment;

    ws.write_string_with_format(row_index, 0, "合计", &total_label_fmt)?;
    ws.merge_range(
        row_index,
        1,
        row_index,
        3,
        &format!(
            "发票金额 {:.2} 元，支付金额 {:.2} 元，优惠金额 {:.2} 元",
            total_invoice, total_payment, total_discount
        ),
        &total_label_fmt,
    )?;
    ws.write_number_with_format(row_index, 4, total_invoice, &total_num_fmt)?;
    ws.write_blank(row_index, 5, &thin)?;
    ws.write_blank(row_index, 6, &thin)?;
    ws.write_number_with_format(row_index, 7, total_payment, &total_num_fmt)?;
    ws.write_number_with_format(row_index, 8, total_discount, &total_num_fmt)?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Helper functions
// ═══════════════════════════════════════════════════════════════════

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

fn payment_source_label(source: &PaymentSource) -> &str {
    match source {
        PaymentSource::Wechat => "微信",
        PaymentSource::Alipay => "支付宝",
    }
}

/// Converts a numeric amount to Chinese uppercase (e.g. 8132.78 → "捌仟壹佰叁拾贰元柒角捌分").
fn amount_to_chinese(amount: f64) -> String {
    if amount.abs() < 0.01 {
        return "零元整".to_string();
    }
    let digits = ["零", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"];
    let units = ["", "拾", "佰", "仟"];
    let big_units = ["", "万", "亿"];

    let amount_cents = (amount * 100.0).round() as i64;
    let yuan = amount_cents / 100;
    let jiao = ((amount_cents % 100) / 10) as usize;
    let fen = (amount_cents % 10) as usize;

    let mut result = String::new();

    if yuan == 0 {
        // less than 1 yuan
    } else {
        let yuan_str = yuan.to_string();
        let chars: Vec<char> = yuan_str.chars().collect();
        let len = chars.len();
        let mut need_zero = false;

        for (i, &ch) in chars.iter().enumerate() {
            let d = ch.to_digit(10).unwrap() as usize;
            let pos = len - 1 - i;
            let unit_idx = pos % 4;
            let big_unit_idx = pos / 4;

            if d == 0 {
                need_zero = true;
            } else {
                if need_zero {
                    result.push('零');
                    need_zero = false;
                }
                result.push_str(digits[d]);
                result.push_str(units[unit_idx]);
            }

            if unit_idx == 0 && big_unit_idx > 0 {
                result.push_str(big_units[big_unit_idx]);
            }
        }
        result.push('元');
    }

    if jiao == 0 && fen == 0 {
        result.push('整');
    } else {
        if jiao > 0 {
            result.push_str(digits[jiao]);
            result.push('角');
        } else if !result.is_empty() {
            result.push('零');
        }
        if fen > 0 {
            result.push_str(digits[fen]);
            result.push('分');
        }
    }

    result
}

// ═══════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{Invoice, InvoiceSource, Itinerary};
    use crate::models::match_result::MatchType;
    use crate::models::payment::{PaymentRecord, PaymentSource};
    use crate::models::reimbursement::{
        HotelLevelDetail, MealSubsidyDetail, ReimbursementForm, TransportDetail,
    };
    use chrono::NaiveDate;

    fn sample_form() -> ReimbursementForm {
        ReimbursementForm {
            name: "张三".to_string(),
            department: "聘用工程师".to_string(),
            destination: "北京→上海".to_string(),
            travel_start: "2025-08-04".to_string(),
            travel_end: "2025-08-15".to_string(),
            travel_days: 12,
            companions: 0,
            transport_details: vec![
                TransportDetail {
                    label: "车、船票".to_string(),
                    count: 1,
                    amount: 553.0,
                },
                TransportDetail {
                    label: "飞机票".to_string(),
                    count: 1,
                    amount: 1090.0,
                },
                TransportDetail {
                    label: "订（退、改签）票".to_string(),
                    count: 1,
                    amount: 110.5,
                },
            ],
            transport_subtotal: 1753.5,
            city_transport_count: 20,
            city_transport_amount: 956.65,
            city_transport_actual_amount: 956.65,
            hotel_levels: vec![HotelLevelDetail {
                level: "其他人员".to_string(),
                persons: 1,
                days: 11,
                daily_rate: 350.0,
                amount: 3850.0,
                actual_amount: 4222.63,
            }],
            hotel_subtotal: 4222.63,
            meal_subsidy: MealSubsidyDetail {
                persons: 1,
                days: 12,
                daily_rate: 100.0,
                amount: 1200.0,
            },
            baggage_amount: 0.0,
            meal_reimbursement: 0.0,
            advance_payment: 2000.0,
            summaries: vec![],
            total_amount: 8132.78,
        }
    }

    fn sample_match_results() -> Vec<MatchResult> {
        vec![
            // Train invoice
            MatchResult {
                invoice_id: "inv-1".to_string(),
                invoice: Invoice {
                    id: "inv-1".to_string(),
                    invoice_number: "TRAIN001".to_string(),
                    amount: 553.0,
                    seller_name: "铁路局".to_string(),
                    item_name: "火车票".to_string(),
                    date: NaiveDate::from_ymd_opt(2025, 8, 4).unwrap(),
                    travel_date: None,
                    category: InvoiceCategory::Train,
                    source: InvoiceSource::Pdf("train.pdf".to_string()),
                    itineraries: vec![],
                    itinerary_file: None,
                    remarks: "D1234 北京→上海".to_string(),
                    hotel_detail: None,
                    departure_city: None,
                    arrival_city: None,
                },
                payment_ids: vec!["pay-train".to_string()],
                payments: vec![PaymentRecord {
                    amount: 553.0,
                    discount: 0.0,
                    source: PaymentSource::Wechat,
                    ..Default::default()
                }],
                match_type: MatchType::OneToOne,
                confidence: 1.0,
                amount_diff: 0.0,
                itinerary_payment_pairs: vec![],
            },
            // City transport with itineraries
            MatchResult {
                invoice_id: "inv-2".to_string(),
                invoice: Invoice {
                    id: "inv-2".to_string(),
                    invoice_number: "DIDI001".to_string(),
                    amount: 75.0,
                    seller_name: "滴滴出行".to_string(),
                    item_name: "网约车".to_string(),
                    date: NaiveDate::from_ymd_opt(2025, 8, 5).unwrap(),
                    travel_date: None,
                    category: InvoiceCategory::CityTransport,
                    source: InvoiceSource::Photo("didi.jpg".to_string()),
                    itineraries: vec![
                        Itinerary {
                            date_time: "08-05 09:15".to_string(),
                            provider: "滴滴".to_string(),
                            pickup: "北京站".to_string(),
                            dropoff: "国贸".to_string(),
                            amount: 35.0,
                        },
                        Itinerary {
                            date_time: "08-05 18:30".to_string(),
                            provider: "滴滴".to_string(),
                            pickup: "国贸".to_string(),
                            dropoff: "北京南站".to_string(),
                            amount: 40.0,
                        },
                    ],
                    itinerary_file: None,
                    remarks: String::new(),
                    hotel_detail: None,
                    departure_city: None,
                    arrival_city: None,
                },
                payment_ids: vec!["pay-didi".to_string()],
                payments: vec![PaymentRecord {
                    amount: 75.0,
                    discount: 3.0,
                    source: PaymentSource::Wechat,
                    ..Default::default()
                }],
                match_type: MatchType::OneToOne,
                confidence: 1.0,
                amount_diff: 0.0,
                itinerary_payment_pairs: vec![],
            },
            // Hotel invoice
            MatchResult {
                invoice_id: "inv-3".to_string(),
                invoice: Invoice {
                    id: "inv-3".to_string(),
                    invoice_number: "HOTEL001".to_string(),
                    amount: 4222.63,
                    seller_name: "上海大酒店".to_string(),
                    item_name: "住宿费".to_string(),
                    date: NaiveDate::from_ymd_opt(2025, 8, 5).unwrap(),
                    travel_date: None,
                    category: InvoiceCategory::Hotel,
                    source: InvoiceSource::Pdf("hotel.pdf".to_string()),
                    itineraries: vec![],
                    itinerary_file: None,
                    remarks: String::new(),
                    hotel_detail: None,
                    departure_city: None,
                    arrival_city: None,
                },
                payment_ids: vec!["pay-hotel".to_string()],
                payments: vec![PaymentRecord {
                    amount: 4222.63,
                    discount: 0.0,
                    source: PaymentSource::Alipay,
                    ..Default::default()
                }],
                match_type: MatchType::OneToOne,
                confidence: 1.0,
                amount_diff: 0.0,
                itinerary_payment_pairs: vec![],
            },
        ]
    }

    #[test]
    fn test_generate_xlsx_file() {
        let form = sample_form();
        let results = sample_match_results();
        let tmp_dir = std::env::temp_dir();
        let output_path = tmp_dir.join("test_reimbursement.xlsx");
        let output_str = output_path.to_str().unwrap();

        let result = generate_reimbursement_xlsx(&form, &results, output_str);
        assert!(result.is_ok(), "XLSX generation failed: {:?}", result.err());

        // File should exist and be non-empty
        assert!(output_path.exists(), "Output file does not exist");
        let metadata = std::fs::metadata(&output_path).unwrap();
        assert!(metadata.len() > 0, "Output file is empty");

        // Clean up
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_amount_to_chinese() {
        assert_eq!(amount_to_chinese(0.0), "零元整");
        assert_eq!(amount_to_chinese(100.0), "壹佰元整");
        assert_eq!(amount_to_chinese(8132.78), "捌仟壹佰叁拾贰元柒角捌分");
        assert_eq!(amount_to_chinese(10.05), "壹拾元零伍分");
        assert_eq!(amount_to_chinese(1.50), "壹元伍角");
    }

    #[test]
    fn test_category_label() {
        assert_eq!(category_label(&InvoiceCategory::Train), "车、船票");
        assert_eq!(category_label(&InvoiceCategory::Flight), "飞机票");
        assert_eq!(
            category_label(&InvoiceCategory::TicketChange),
            "订（退、改签）票及交通保险费"
        );
        assert_eq!(
            category_label(&InvoiceCategory::CityTransport),
            "市内交通费"
        );
        assert_eq!(category_label(&InvoiceCategory::Hotel), "住宿费");
    }

    #[test]
    fn test_payment_source_label() {
        assert_eq!(payment_source_label(&PaymentSource::Wechat), "微信");
        assert_eq!(payment_source_label(&PaymentSource::Alipay), "支付宝");
    }
}
