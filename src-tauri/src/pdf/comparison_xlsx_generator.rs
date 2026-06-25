use crate::models::invoice::{HotelDetail, Invoice, InvoiceCategory};
use crate::models::match_result::{MatchResult, MatchType};
use crate::models::payment::{PaymentRecord, PaymentSource};
use chrono::NaiveDateTime;
use rust_xlsxwriter::*;
use std::error::Error;

/// Generates a comprehensive Excel comparison sheet containing all information
/// from invoices, itineraries, and payments in one wide table.
///
/// The resulting sheet ("完整信息对照单") contains 30 columns (A–AD) with:
/// - One row per match result (expanded into multiple rows for CityTransport with itineraries)
/// - All invoice fields, payment fields, itinerary fields, hotel fields, and match metadata
/// - A title row, header row, and footer with totals
pub fn generate_comparison_xlsx(
    match_results: &[MatchResult],
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let mut workbook = Workbook::new();
    let ws = workbook.add_worksheet();
    ws.set_name("完整信息对照单")?;

    build_comparison_sheet(ws, match_results)?;

    workbook.save(output_path)?;
    Ok(())
}

/// Last column index (0-based): AD = 29
const LAST_COL: u16 = 29;

fn build_comparison_sheet(
    ws: &mut Worksheet,
    match_results: &[MatchResult],
) -> Result<(), Box<dyn Error>> {
    // ── Column widths (A–AD, cols 0–29) ──
    let col_widths: [(u16, f64); 30] = [
        (0, 5.0),
        (1, 18.0),
        (2, 12.0),
        (3, 10.0),
        (4, 22.0),
        (5, 20.0),
        (6, 12.0),
        (7, 18.0),
        (8, 26.0),
        (9, 18.0),
        (10, 10.0),
        (11, 10.0),
        (12, 10.0),
        (13, 10.0),
        (14, 22.0),
        (15, 12.0),
        (16, 18.0),
        (17, 12.0),
        (18, 18.0),
        (19, 18.0),
        (20, 10.0),
        (21, 12.0),
        (22, 12.0),
        (23, 8.0),
        (24, 10.0),
        (25, 14.0),
        (26, 10.0),
        (27, 10.0),
        (28, 10.0),
        (29, 12.0),
    ];
    for (col, width) in col_widths {
        ws.set_column_width(col, width)?;
    }

    // ── Reusable formats ──
    let thin = Format::new().set_border(FormatBorder::Thin);

    let title_fmt = Format::new()
        .set_bold()
        .set_font_size(14)
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    let header_fmt = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_background_color(Color::RGB(0xE8E8E8));

    let text_center_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    let text_left_wrap_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_text_wrap()
        .set_align(FormatAlign::VerticalCenter);

    let text_center_wrap_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_text_wrap()
        .set_align(FormatAlign::Center);

    let num_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_num_format("0.00")
        .set_align(FormatAlign::VerticalCenter);

    let num_center_fmt = Format::new()
        .set_border(FormatBorder::Thin)
        .set_num_format("0.00")
        .set_align(FormatAlign::Center);

    let footer_label_fmt = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    let footer_num_fmt = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_num_format("0.00")
        .set_align(FormatAlign::VerticalCenter);

    // ═══════════════════════════════════════════════════════════════
    //  Row 0 — Title (merged A1:AD1)
    // ═══════════════════════════════════════════════════════════════
    ws.merge_range(0, 0, 0, LAST_COL, "完整信息对照单", &title_fmt)?;
    ws.set_row_height(0, 32)?;

    // ═══════════════════════════════════════════════════════════════
    //  Row 1 — Headers
    // ═══════════════════════════════════════════════════════════════
    let headers: [(u16, &str); 30] = [
        (0, "序号"),
        (1, "发票号码"),
        (2, "发票类别"),
        (3, "发票金额"),
        (4, "销售方"),
        (5, "项目名称"),
        (6, "开票日期"),
        (7, "支付渠道"),
        (8, "支付单号"),
        (9, "支付时间"),
        (10, "实付金额"),
        (11, "原始金额"),
        (12, "退款"),
        (13, "优惠"),
        (14, "商户名称"),
        (15, "支付方式"),
        (16, "行程时间"),
        (17, "服务商"),
        (18, "上车点"),
        (19, "下车点"),
        (20, "行程金额"),
        (21, "入住日期"),
        (22, "离店日期"),
        (23, "住宿天数"),
        (24, "每晚均价"),
        (25, "发票备注"),
        (26, "匹配类型"),
        (27, "金额差异"),
        (28, "置信度"),
        (29, "时间差异"),
    ];
    for (col, hdr) in headers {
        ws.write_string_with_format(1, col, hdr, &header_fmt)?;
    }
    ws.set_row_height(1, 26)?;

    // ═══════════════════════════════════════════════════════════════
    //  Rows 2+ — Data
    // ═══════════════════════════════════════════════════════════════
    let mut xlsx_row: u32 = 2;
    let mut seq: u32 = 1;

    for result in match_results {
        let invoice = &result.invoice;
        let is_city_transport = matches!(invoice.category, InvoiceCategory::CityTransport);
        let has_itineraries = is_city_transport && !invoice.itineraries.is_empty();

        if has_itineraries {
            // ── CityTransport: one row per itinerary ──
            for (i, itinerary) in invoice.itineraries.iter().enumerate() {
                let r = xlsx_row;
                xlsx_row += 1;

                // Col 0 — 序号 (every itinerary gets its own sequence number)
                ws.write_number_with_format(r, 0, seq as f64, &text_center_fmt)?;
                seq += 1;

                // Invoice fields — first row only
                if i == 0 {
                    write_invoice_fields(ws, r, invoice, &text_center_fmt, &num_fmt)?;
                } else {
                    write_blank_range(ws, r, 1..=6, &thin)?;
                }

                // Payment fields — match by index (payment[i] → itinerary[i])
                if let Some(payment) = result.payment_for_itinerary(i) {
                    write_payment_fields(ws, r, payment, &text_center_fmt, &text_center_wrap_fmt, &num_fmt)?;
                } else {
                    write_blank_range(ws, r, 7..=15, &thin)?;
                }

                // Itinerary fields
                ws.write_string_with_format(r, 16, &itinerary.date_time, &text_center_fmt)?;
                ws.write_string_with_format(r, 17, &itinerary.provider, &text_center_fmt)?;
                write_string_or_blank(ws, r, 18, &itinerary.pickup, &text_left_wrap_fmt)?;
                write_string_or_blank(ws, r, 19, &itinerary.dropoff, &text_left_wrap_fmt)?;
                ws.write_number_with_format(r, 20, itinerary.amount, &num_fmt)?;

                // Hotel fields — blank (CityTransport has no hotel data)
                write_blank_range(ws, r, 21..=24, &thin)?;

                // 发票备注 — first row only
                if i == 0 {
                    write_string_or_blank(ws, r, 25, &invoice.remarks, &text_left_wrap_fmt)?;
                } else {
                    ws.write_blank(r, 25, &thin)?;
                }

                // 匹配类型 / 金额差异 / 置信度 — first row only
                if i == 0 {
                    write_match_fields(ws, r, result, &text_center_fmt, &num_center_fmt)?;
                } else {
                    write_blank_range(ws, r, 26..=28, &thin)?;
                }

                // 时间差异 — 每个行程与对应支付的时间差
                {
                    let time_diff = if let Some(payment) = result.payment_for_itinerary(i) {
                        compute_time_diff(&itinerary.date_time, &payment.transaction_time)
                    } else {
                        String::new()
                    };
                    if !time_diff.is_empty() {
                        ws.write_string_with_format(r, 29, &time_diff, &text_center_fmt)?;
                    } else {
                        ws.write_blank(r, 29, &thin)?;
                    }
                }
            }
        } else {
            // ── Single row for non-itinerary match result ──
            let r = xlsx_row;
            xlsx_row += 1;

            // Col 0 — 序号
            ws.write_number_with_format(r, 0, seq as f64, &text_center_fmt)?;
            seq += 1;

            // Invoice fields
            write_invoice_fields(ws, r, invoice, &text_center_fmt, &num_fmt)?;

            // Payment fields
            if result.payments.is_empty() {
                write_blank_range(ws, r, 7..=15, &thin)?;
            } else if result.payments.len() == 1 {
                // Single payment: show directly
                write_payment_fields(ws, r, &result.payments[0], &text_center_fmt, &text_center_wrap_fmt, &num_fmt)?;
            } else {
                // Multiple payments: show first, join all transaction IDs
                write_payment_fields(ws, r, &result.payments[0], &text_center_fmt, &text_center_wrap_fmt, &num_fmt)?;
                let ids: Vec<&str> = result.payments.iter().map(|p| p.transaction_id.as_str()).collect();
                ws.write_string_with_format(r, 8, &ids.join(", "), &text_center_wrap_fmt)?;
            }

            // Itinerary fields — blank (not CityTransport with itineraries)
            write_blank_range(ws, r, 16..=20, &thin)?;

            // Hotel fields
            if let Some(hotel) = &invoice.hotel_detail {
                write_hotel_fields(ws, r, hotel, &text_center_fmt, &num_fmt)?;
            } else {
                write_blank_range(ws, r, 21..=24, &thin)?;
            }

            // 发票备注
            write_string_or_blank(ws, r, 25, &invoice.remarks, &text_left_wrap_fmt)?;

            // 匹配类型 / 金额差异 / 置信度
            write_match_fields(ws, r, result, &text_center_fmt, &num_center_fmt)?;
            ws.write_blank(r, 29, &thin)?;
        }
    }

    // ═══════════════════════════════════════════════════════════════
    //  Footer — blank separator then 合计
    // ═══════════════════════════════════════════════════════════════
    xlsx_row += 1; // blank separator row

    let footer_row = xlsx_row;
    let total_count = seq - 1;

    // Compute totals
    let total_invoice_amount: f64 = match_results.iter().map(|r| r.invoice.amount).sum();
    let total_paid: f64 = match_results
        .iter()
        .flat_map(|r| &r.payments)
        .map(|p| p.amount)
        .sum();
    let total_refund: f64 = match_results
        .iter()
        .flat_map(|r| &r.payments)
        .map(|p| p.refund_amount)
        .sum();
    let total_discount: f64 = match_results
        .iter()
        .flat_map(|r| &r.payments)
        .map(|p| p.discount)
        .sum();
    let total_itinerary_amount: f64 = match_results
        .iter()
        .flat_map(|r| &r.invoice.itineraries)
        .map(|i| i.amount)
        .sum();

    // Col 0: "合计"
    ws.write_string_with_format(footer_row, 0, "合计", &footer_label_fmt)?;

    // Col 1: record count
    ws.write_string_with_format(
        footer_row,
        1,
        &format!("共 {} 条记录", total_count),
        &footer_label_fmt,
    )?;

    // Col 2: blank
    ws.write_blank(footer_row, 2, &thin)?;

    // Col 3: total invoice amount
    ws.write_number_with_format(footer_row, 3, total_invoice_amount, &footer_num_fmt)?;

    // Cols 4–9: blank
    write_blank_range(ws, footer_row, 4..=9, &thin)?;

    // Col 10: total paid
    ws.write_number_with_format(footer_row, 10, total_paid, &footer_num_fmt)?;

    // Col 11: blank
    ws.write_blank(footer_row, 11, &thin)?;

    // Col 12: total refund
    ws.write_number_with_format(footer_row, 12, total_refund, &footer_num_fmt)?;

    // Col 13: total discount
    ws.write_number_with_format(footer_row, 13, total_discount, &footer_num_fmt)?;

    // Cols 14–19: blank
    write_blank_range(ws, footer_row, 14..=19, &thin)?;

    // Col 20: total itinerary amount
    ws.write_number_with_format(footer_row, 20, total_itinerary_amount, &footer_num_fmt)?;

    // Cols 21–29: blank
    write_blank_range(ws, footer_row, 21..=LAST_COL, &thin)?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Field writers
// ═══════════════════════════════════════════════════════════════════

/// Writes cols 1–6: invoice_number, category, amount, seller_name, item_name, date.
fn write_invoice_fields(
    ws: &mut Worksheet,
    row: u32,
    invoice: &Invoice,
    center_fmt: &Format,
    amt_fmt: &Format,
) -> Result<(), Box<dyn Error>> {
    ws.write_string_with_format(row, 1, &invoice.invoice_number, center_fmt)?;
    ws.write_string_with_format(row, 2, category_label(&invoice.category), center_fmt)?;
    ws.write_number_with_format(row, 3, invoice.amount, amt_fmt)?;
    ws.write_string_with_format(row, 4, &invoice.seller_name, center_fmt)?;
    ws.write_string_with_format(row, 5, &invoice.item_name, center_fmt)?;
    let date_str = invoice.date.format("%Y-%m-%d").to_string();
    ws.write_string_with_format(row, 6, &date_str, center_fmt)?;
    Ok(())
}

/// Writes cols 7–15: source, transaction_id, transaction_time, amount,
/// original_amount, refund_amount, discount, merchant_name, payment_method.
fn write_payment_fields(
    ws: &mut Worksheet,
    row: u32,
    payment: &PaymentRecord,
    center_fmt: &Format,
    id_fmt: &Format,
    amt_fmt: &Format,
) -> Result<(), Box<dyn Error>> {
    ws.write_string_with_format(row, 7, payment_source_label(&payment.source), center_fmt)?;
    ws.write_string_with_format(row, 8, &payment.transaction_id, id_fmt)?;
    ws.write_string_with_format(row, 9, &payment.transaction_time, center_fmt)?;
    ws.write_number_with_format(row, 10, payment.amount, amt_fmt)?;
    ws.write_number_with_format(row, 11, payment.original_amount, amt_fmt)?;
    ws.write_number_with_format(row, 12, payment.refund_amount, amt_fmt)?;
    ws.write_number_with_format(row, 13, payment.discount, amt_fmt)?;
    ws.write_string_with_format(row, 14, &payment.merchant_name, center_fmt)?;
    ws.write_string_with_format(row, 15, &payment.payment_method, center_fmt)?;
    Ok(())
}

/// Writes cols 21–24: check_in, check_out, nights, nightly_rate.
fn write_hotel_fields(
    ws: &mut Worksheet,
    row: u32,
    hotel: &HotelDetail,
    center_fmt: &Format,
    amt_fmt: &Format,
) -> Result<(), Box<dyn Error>> {
    let check_in = hotel
        .check_in
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let check_out = hotel
        .check_out
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    ws.write_string_with_format(row, 21, &check_in, center_fmt)?;
    ws.write_string_with_format(row, 22, &check_out, center_fmt)?;
    ws.write_number_with_format(row, 23, hotel.nights as f64, center_fmt)?;
    ws.write_number_with_format(row, 24, hotel.nightly_rate, amt_fmt)?;
    Ok(())
}

/// Writes cols 26–28: match_type, amount_diff, confidence.
fn write_match_fields(
    ws: &mut Worksheet,
    row: u32,
    result: &MatchResult,
    center_fmt: &Format,
    num_fmt: &Format,
) -> Result<(), Box<dyn Error>> {
    ws.write_string_with_format(row, 26, match_type_label(&result.match_type), center_fmt)?;
    ws.write_number_with_format(row, 27, result.amount_diff, num_fmt)?;
    ws.write_number_with_format(row, 28, result.confidence, center_fmt)?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Utility helpers
// ═══════════════════════════════════════════════════════════════════

/// Writes a string (or blank if empty) into the given cell.
fn write_string_or_blank(
    ws: &mut Worksheet,
    row: u32,
    col: u16,
    text: &str,
    fmt: &Format,
) -> Result<(), Box<dyn Error>> {
    if text.is_empty() {
        ws.write_blank(row, col, fmt)?;
    } else {
        ws.write_string_with_format(row, col, text, fmt)?;
    }
    Ok(())
}

/// Writes blank cells for a range of columns (inclusive range).
fn write_blank_range(
    ws: &mut Worksheet,
    row: u32,
    cols: std::ops::RangeInclusive<u16>,
    fmt: &Format,
) -> Result<(), Box<dyn Error>> {
    for col in cols {
        ws.write_blank(row, col, fmt)?;
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Label helpers
// ═══════════════════════════════════════════════════════════════════

fn category_label(cat: &InvoiceCategory) -> &str {
    match cat {
        InvoiceCategory::Train => "车、船票",
        InvoiceCategory::Flight => "飞机票",
        InvoiceCategory::TicketChange => "订（退、改签）票及交通保险费",
        InvoiceCategory::CityTransport => "市内交通费",
        InvoiceCategory::Toll => "市内交通费",
        InvoiceCategory::Hotel => "住宿费",
        InvoiceCategory::Meal => "餐补/伙食补助",
        InvoiceCategory::Other => "其他",
    }
}

fn match_type_label(mt: &MatchType) -> &str {
    match mt {
        MatchType::OneToOne => "一对一匹配",
        MatchType::OneToMany => "一对多匹配",
        MatchType::Unmatched => "未匹配",
        MatchType::ManualConfirmed => "手动确认",
    }
}

fn payment_source_label(source: &PaymentSource) -> &str {
    match source {
        PaymentSource::Wechat => "微信",
        PaymentSource::Alipay => "支付宝",
    }
}

/// Calculate the time difference between an itinerary time and a payment time.
/// Returns a human-readable string like "23分钟" or "1小时15分", or empty string on parse failure.
fn compute_time_diff(itinerary_time: &str, payment_time: &str) -> String {
    // Parse payment time first to get the year
    let pay_dt = {
        let mut result = None;
        // Try "YYYY-MM-DD HH:MM:SS"
        if result.is_none() {
            result = NaiveDateTime::parse_from_str(payment_time, "%Y-%m-%d %H:%M:%S").ok();
        }
        // Try "YYYY-MM-DD HH:MM" (WeChat format, no seconds)
        if result.is_none() {
            result = NaiveDateTime::parse_from_str(
                &format!("{}:00", payment_time),
                "%Y-%m-%d %H:%M:%S"
            ).ok();
        }
        // Try "YYYY/MM/DD HH:MM:SS"
        if result.is_none() {
            result = NaiveDateTime::parse_from_str(payment_time, "%Y/%m/%d %H:%M:%S").ok();
        }
        // Try "YYYY/MM/DD HH:MM" (no seconds)
        if result.is_none() {
            result = NaiveDateTime::parse_from_str(
                &format!("{}:00", payment_time),
                "%Y/%m/%d %H:%M:%S"
            ).ok();
        }
        match result {
            Some(dt) => dt,
            None => return String::new(),
        }
    };

    let pay_year = pay_dt.format("%Y").to_string();

    // Parse itinerary time, using the payment year for formats that lack a year
    let itin_dt = {
        let mut result = None;
        // Try "MM-DD HH:MM" with payment year
        if result.is_none() {
            result = NaiveDateTime::parse_from_str(
                &format!("{}-{}:00", pay_year, itinerary_time),
                "%Y-%m-%d %H:%M:%S"
            ).ok();
        }
        // Try "YYYY-MM-DD HH:MM:SS"
        if result.is_none() {
            result = NaiveDateTime::parse_from_str(itinerary_time, "%Y-%m-%d %H:%M:%S").ok();
        }
        // Try "MM-DD HH:MM:SS" with payment year
        if result.is_none() {
            result = NaiveDateTime::parse_from_str(
                &format!("{}-{}", pay_year, itinerary_time),
                "%Y-%m-%d %H:%M:%S"
            ).ok();
        }
        match result {
            Some(dt) => dt,
            None => return String::new(),
        }
    };

    let duration = (pay_dt - itin_dt).num_minutes().abs();

    if duration < 60 {
        format!("{}分钟", duration)
    } else {
        let hours = duration / 60;
        let mins = duration % 60;
        if mins == 0 {
            format!("{}小时", hours)
        } else {
            format!("{}小时{}分", hours, mins)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{Invoice, InvoiceSource, Itinerary};
    use crate::models::match_result::MatchType;
    use crate::models::payment::PaymentRecord;
    use chrono::NaiveDate;

    fn sample_match_results() -> Vec<MatchResult> {
        vec![
            // Train invoice — one-to-one
            MatchResult {
                invoice_id: "inv-1".to_string(),
                invoice: Invoice {
                    id: "inv-1".to_string(),
                    invoice_number: "TRAIN001".to_string(),
                    amount: 553.0,
                    seller_name: "中国铁路局".to_string(),
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
                                toll_travel_time: None,
                },
                payment_ids: vec!["pay-train".to_string()],
                payments: vec![PaymentRecord {
                    id: "pay-train".to_string(),
                    transaction_id: "WX20250804123456".to_string(),
                    transaction_time: "2025-08-04 10:00".to_string(),
                    amount: 553.0,
                    original_amount: 553.0,
                    refund_amount: 0.0,
                    discount: 0.0,
                    merchant_name: "中国铁路局".to_string(),
                    source: PaymentSource::Wechat,
                    category: "交通".to_string(),
                    payment_method: "零钱".to_string(),
                }],
                match_type: MatchType::OneToOne,
                confidence: 1.0,
                amount_diff: 0.0,
                itinerary_payment_pairs: vec![],
            },
            // City transport with 2 itineraries, 1 payment
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
                                toll_travel_time: None,
                },
                payment_ids: vec!["pay-didi".to_string()],
                payments: vec![PaymentRecord {
                    id: "pay-didi".to_string(),
                    transaction_id: "WX20250805123456".to_string(),
                    transaction_time: "2025-08-05 10:00".to_string(),
                    amount: 75.0,
                    original_amount: 78.0,
                    refund_amount: 0.0,
                    discount: 3.0,
                    merchant_name: "滴滴出行".to_string(),
                    source: PaymentSource::Wechat,
                    category: "交通".to_string(),
                    payment_method: "零钱".to_string(),
                }],
                match_type: MatchType::OneToOne,
                confidence: 1.0,
                amount_diff: 0.0,
                itinerary_payment_pairs: vec![],
            },
            // Hotel invoice with details
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
                    hotel_detail: Some(HotelDetail {
                        check_in: Some(NaiveDate::from_ymd_opt(2025, 8, 5).unwrap()),
                        check_out: Some(NaiveDate::from_ymd_opt(2025, 8, 11).unwrap()),
                        nights: 6,
                        nightly_rate: 703.77,
                    }),
                    departure_city: None,
                    arrival_city: None,
                                toll_travel_time: None,
                },
                payment_ids: vec!["pay-hotel".to_string()],
                payments: vec![PaymentRecord {
                    id: "pay-hotel".to_string(),
                    transaction_id: "ALIPAY20250805123456".to_string(),
                    transaction_time: "2025-08-05 14:30".to_string(),
                    amount: 4222.63,
                    original_amount: 4222.63,
                    refund_amount: 0.0,
                    discount: 0.0,
                    merchant_name: "上海大酒店".to_string(),
                    source: PaymentSource::Alipay,
                    category: "住宿".to_string(),
                    payment_method: "余额宝".to_string(),
                }],
                match_type: MatchType::OneToOne,
                confidence: 1.0,
                amount_diff: 0.0,
                itinerary_payment_pairs: vec![],
            },
            // Unmatched invoice
            MatchResult {
                invoice_id: "inv-4".to_string(),
                invoice: Invoice {
                    id: "inv-4".to_string(),
                    invoice_number: "UNMATCHED001".to_string(),
                    amount: 100.0,
                    seller_name: "未知商户".to_string(),
                    item_name: "其他".to_string(),
                    date: NaiveDate::from_ymd_opt(2025, 8, 10).unwrap(),
                    travel_date: None,
                    category: InvoiceCategory::Other,
                    source: InvoiceSource::Photo("other.jpg".to_string()),
                    itineraries: vec![],
                    itinerary_file: None,
                    remarks: "无匹配发票".to_string(),
                    hotel_detail: None,
                    departure_city: None,
                    arrival_city: None,
                                toll_travel_time: None,
                },
                payment_ids: vec![],
                payments: vec![],
                match_type: MatchType::Unmatched,
                confidence: 0.0,
                amount_diff: 100.0,
                itinerary_payment_pairs: vec![],
            },
        ]
    }

    #[test]
    fn test_generate_comparison_xlsx_file() {
        let results = sample_match_results();
        let tmp_dir = std::env::temp_dir();
        let output_path = tmp_dir.join("test_comparison.xlsx");
        let output_str = output_path.to_str().unwrap();

        let result = generate_comparison_xlsx(&results, output_str);
        assert!(
            result.is_ok(),
            "XLSX generation failed: {:?}",
            result.err()
        );

        // File should exist and be non-empty
        assert!(output_path.exists(), "Output file does not exist");
        let metadata = std::fs::metadata(&output_path).unwrap();
        assert!(metadata.len() > 0, "Output file is empty");

        // Clean up
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_generate_empty_results() {
        let results: Vec<MatchResult> = vec![];
        let tmp_dir = std::env::temp_dir();
        let output_path = tmp_dir.join("test_comparison_empty.xlsx");
        let output_str = output_path.to_str().unwrap();

        let result = generate_comparison_xlsx(&results, output_str);
        assert!(
            result.is_ok(),
            "XLSX generation with empty results failed: {:?}",
            result.err()
        );

        assert!(output_path.exists(), "Output file does not exist");
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_category_label() {
        assert_eq!(category_label(&InvoiceCategory::Train), "车、船票");
        assert_eq!(category_label(&InvoiceCategory::Flight), "飞机票");
        assert_eq!(
            category_label(&InvoiceCategory::TicketChange),
            "订（退、改签）票及交通保险费"
        );
        assert_eq!(category_label(&InvoiceCategory::CityTransport), "市内交通费");
        assert_eq!(category_label(&InvoiceCategory::Hotel), "住宿费");
        assert_eq!(category_label(&InvoiceCategory::Meal), "餐补/伙食补助");
        assert_eq!(category_label(&InvoiceCategory::Other), "其他");
    }

    #[test]
    fn test_match_type_label() {
        assert_eq!(match_type_label(&MatchType::OneToOne), "一对一匹配");
        assert_eq!(match_type_label(&MatchType::OneToMany), "一对多匹配");
        assert_eq!(match_type_label(&MatchType::Unmatched), "未匹配");
        assert_eq!(match_type_label(&MatchType::ManualConfirmed), "手动确认");
    }

    #[test]
    fn test_payment_source_label() {
        assert_eq!(payment_source_label(&PaymentSource::Wechat), "微信");
        assert_eq!(payment_source_label(&PaymentSource::Alipay), "支付宝");
    }
}
