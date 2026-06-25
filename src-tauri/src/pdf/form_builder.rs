use crate::models::match_result::MatchResult;
use crate::models::invoice::InvoiceCategory;
use crate::models::hotel_standard::get_hotel_nightly_rate_std;
use crate::models::reimbursement::{
    ReimbursementForm, CategorySummary, TransportDetail, HotelLevelDetail, MealSubsidyDetail,
};
use chrono::NaiveDate;
use std::collections::HashMap;

fn days_between(start: &str, end: &str) -> usize {
    let parse = |s: &str| -> Option<NaiveDate> {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
    };
    match (parse(start), parse(end)) {
        (Some(s), Some(e)) => {
            let diff = (e - s).num_days();
            if diff >= 0 { (diff + 1) as usize } else { 1 }
        }
        _ => 1,
    }
}

/// 从匹配结果构建报销表单（完整版，包含所有明细）
pub fn build_reimbursement_form(
    match_results: &[MatchResult],
    name: &str,
    department: &str,
    destination: &str,
    travel_start: &str,
    travel_end: &str,
    companions: usize,
    hotel_level: &str,
) -> ReimbursementForm {
    let travel_days = days_between(travel_start, travel_end);
    let meal_subsidy_rate: f64 = 100.0;
    let nightly_rate_std = get_hotel_nightly_rate_std(destination);

    // 按类别汇总
    let mut category_map: HashMap<InvoiceCategory, (usize, f64)> = HashMap::new();
    for result in match_results {
        let cat = &result.invoice.category;
        let entry = category_map.entry(cat.clone()).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += result.invoice.amount;
    }

    // 城市间交通费明细
    let transport_labels = [
        (InvoiceCategory::Train, "车、船票"),
        (InvoiceCategory::Flight, "飞机票"),
        (InvoiceCategory::TicketChange, "订（退、改签）票"),
    ];
    let mut transport_details = Vec::new();
    let mut transport_subtotal = 0.0;
    for (cat, label) in &transport_labels {
        if let Some(&(count, amount)) = category_map.get(cat) {
            transport_details.push(TransportDetail {
                label: label.to_string(),
                count,
                amount,
            });
            transport_subtotal += amount;
        }
    }

    // 市内交通费（含往返当日，平均每天不超过 80 元）
    let city_transport_raw = category_map
        .get(&InvoiceCategory::CityTransport)
        .copied()
        .unwrap_or((0, 0.0));
    let city_transport_count = city_transport_raw.0;
    let city_transport_actual_amount = city_transport_raw.1;
    let city_transport_daily_std: f64 = 80.0;
    let city_transport_max = city_transport_daily_std * travel_days as f64;
    let city_transport_amount = city_transport_actual_amount.min(city_transport_max);

    // 住宿费：从每张酒店发票的 hotel_detail 获取天数，按标准封顶
    let hotel_invoices: Vec<&MatchResult> = match_results
        .iter()
        .filter(|r| r.invoice.category == InvoiceCategory::Hotel)
        .collect();

    let mut total_hotel_nights: usize = 0;
    let mut total_hotel_actual: f64 = 0.0;
    let mut total_hotel_reimbursable: f64 = 0.0;

    for result in &hotel_invoices {
        let inv = &result.invoice;
        let (nights, actual_amount) = if let Some(ref detail) = inv.hotel_detail {
            (detail.nights, inv.amount)
        } else {
            // 无 hotel_detail 时，用 travel_days - 1 作为估算
            let est_nights = if travel_days > 1 { travel_days - 1 } else { 1 };
            (est_nights, inv.amount)
        };
        total_hotel_nights += nights;
        total_hotel_actual += actual_amount;

        // 按标准封顶
        let standard_amount = nightly_rate_std * nights as f64;
        total_hotel_reimbursable += actual_amount.min(standard_amount);
    }

    let hotel_levels = if !hotel_invoices.is_empty() {
        let _avg_daily_rate = if total_hotel_nights > 0 {
            total_hotel_actual / total_hotel_nights as f64
        } else {
            0.0
        };
        vec![HotelLevelDetail {
            level: hotel_level.to_string(),
            persons: 1,
            days: total_hotel_nights,
            daily_rate: nightly_rate_std,
            amount: total_hotel_reimbursable,
            actual_amount: total_hotel_actual,
        }]
    } else {
        vec![]
    };

    // 伙食补助
    let meal_subsidy_amount = travel_days as f64 * meal_subsidy_rate;
    let meal_subsidy = MealSubsidyDetail {
        persons: 1,
        days: travel_days,
        daily_rate: meal_subsidy_rate,
        amount: meal_subsidy_amount,
    };

    // 兼容旧接口的 summaries
    let order = [
        InvoiceCategory::Train,
        InvoiceCategory::Flight,
        InvoiceCategory::TicketChange,
        InvoiceCategory::CityTransport,
        InvoiceCategory::Hotel,
        InvoiceCategory::Meal,
        InvoiceCategory::Other,
    ];
    let mut summaries: Vec<CategorySummary> = category_map
        .into_iter()
        .map(|(category, (count, total_amount))| CategorySummary {
            category,
            count,
            total_amount,
        })
        .collect();
    summaries.sort_by_key(|s| order.iter().position(|c| c == &s.category).unwrap_or(99));

    // total_amount = 所有发票可报销金额 + 伙食补助
    let invoice_total: f64 = match_results.iter().map(|r| {
        if r.invoice.category == InvoiceCategory::Hotel {
            // 酒店用封顶后的金额
            let nights = r.invoice.hotel_detail.as_ref()
                .map(|d| d.nights)
                .unwrap_or(if travel_days > 1 { travel_days - 1 } else { 1 });
            let standard = nightly_rate_std * nights as f64;
            r.invoice.amount.min(standard)
        } else if r.invoice.category == InvoiceCategory::CityTransport {
            // 市内交通费统一按总封顶计算，不由单张发票累加
            0.0
        } else {
            r.invoice.amount
        }
    }).sum();
    let invoice_total = invoice_total + city_transport_amount;
    let total_amount = invoice_total + meal_subsidy_amount;

    ReimbursementForm {
        name: name.to_string(),
        department: department.to_string(),
        destination: destination.to_string(),
        travel_start: travel_start.to_string(),
        travel_end: travel_end.to_string(),
        travel_days,
        companions,
        transport_details,
        transport_subtotal,
        city_transport_count,
        city_transport_amount,
        city_transport_actual_amount,
        hotel_levels,
        hotel_subtotal: total_hotel_reimbursable,
        meal_subsidy,
        baggage_amount: 0.0,
        meal_reimbursement: 0.0,
        advance_payment: 0.0,
        summaries,
        total_amount,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{Invoice, InvoiceSource, HotelDetail};
    use crate::models::match_result::MatchType;
    use crate::models::payment::{PaymentRecord, PaymentSource};
    use chrono::NaiveDate;

    fn make_invoice(id: &str, amount: f64, category: InvoiceCategory) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "Test Seller".to_string(),
            item_name: "Test Item".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 8, 4).unwrap(),
            travel_date: None,
            category,
            source: InvoiceSource::Pdf("test.pdf".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
        }
    }

    fn make_hotel_invoice(id: &str, amount: f64, nights: usize) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "Test Hotel".to_string(),
            item_name: "*住宿服务*住宿费".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 8, 4).unwrap(),
            travel_date: None,
            category: InvoiceCategory::Hotel,
            source: InvoiceSource::Pdf("test.pdf".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: format!("订单日期:8-04至8-{:02},共{}天,共1间", 4 + nights as u32, nights),
            hotel_detail: Some(HotelDetail {
                check_in: Some(NaiveDate::from_ymd_opt(2025, 8, 4).unwrap()),
                check_out: Some(NaiveDate::from_ymd_opt(2025, 8, 4 + nights as u32).unwrap()),
                nights,
                nightly_rate: amount / nights as f64,
            }),
            departure_city: None,
            arrival_city: None,
        }
    }

    fn make_match_result_from_invoice(invoice: Invoice) -> MatchResult {
        let payment = PaymentRecord {
            id: format!("pay-{}", invoice.id),
            transaction_id: format!("TX-{}", invoice.id),
            transaction_time: "2025-08-04 12:00".to_string(),
            amount: invoice.amount,
            original_amount: invoice.amount,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: "Test".to_string(),
            source: PaymentSource::Wechat,
            category: "交通".to_string(),
            payment_method: String::new(),
        };
        MatchResult {
            invoice_id: invoice.id.clone(),
            invoice,
            payment_ids: vec![payment.id.clone()],
            payments: vec![payment],
            match_type: MatchType::OneToOne,
            confidence: 1.0,
            amount_diff: 0.0,
            itinerary_payment_pairs: vec![],
        }
    }

    #[test]
    fn test_build_form_with_hotel_detail() {
        // 住宿11晚，实际总额 4222.63，成都→四川省标准 370/晚 → 封顶 370*11=4070
        let hotel_inv = make_hotel_invoice("inv-hotel", 4222.63, 11);
        let results = vec![
            make_match_result_from_invoice(make_invoice("inv1", 553.0, InvoiceCategory::Train)),
            make_match_result_from_invoice(hotel_inv),
        ];
        let form = build_reimbursement_form(
            &results, "", "", "成都", "2025-08-04", "2025-08-15", 0, "其他人员",
        );
        assert_eq!(form.travel_days, 12);
        assert_eq!(form.hotel_levels.len(), 1);
        assert_eq!(form.hotel_levels[0].days, 11);
        // 实际 4222.63 > 标准 370*11=4070，应封顶
        assert!((form.hotel_levels[0].actual_amount - 4222.63).abs() < 0.01);
        assert!((form.hotel_levels[0].amount - 4070.0).abs() < 0.01);
        // total_amount = 553 + 4070 + 1200(伙食补助)
        assert!((form.total_amount - 5823.0).abs() < 0.01);
    }

    #[test]
    fn test_build_form_hotel_under_standard() {
        // 住宿5晚，实际总额 1500，标准 350/晚 → 封顶 350*5=1750，不触发封顶
        let hotel_inv = make_hotel_invoice("inv-hotel", 1500.0, 5);
        let results = vec![make_match_result_from_invoice(hotel_inv)];
        let form = build_reimbursement_form(
            &results, "", "", "", "2025-08-04", "2025-08-09", 0, "其他人员",
        );
        assert_eq!(form.hotel_levels[0].days, 5);
        assert!((form.hotel_levels[0].amount - 1500.0).abs() < 0.01);
        assert!((form.hotel_levels[0].actual_amount - 1500.0).abs() < 0.01);
    }

    #[test]
    fn test_city_transport_under_cap() {
        // 出差6天, 市内交通合计 400 < 80*6=480, 不封顶
        let results = vec![
            make_match_result_from_invoice(make_invoice("inv1", 553.0, InvoiceCategory::Train)),
            make_match_result_from_invoice(make_invoice("inv2", 200.0, InvoiceCategory::CityTransport)),
            make_match_result_from_invoice(make_invoice("inv3", 200.0, InvoiceCategory::CityTransport)),
        ];
        let form = build_reimbursement_form(
            &results, "", "", "", "2025-08-04", "2025-08-09", 0, "其他人员",
        );
        assert_eq!(form.travel_days, 6);
        assert_eq!(form.city_transport_count, 2);
        assert!((form.city_transport_amount - 400.0).abs() < 0.01);
        // total = 553 + 400 + 600(伙食) = 1553
        assert!((form.total_amount - 1553.0).abs() < 0.01);
    }

    #[test]
    fn test_city_transport_over_cap() {
        // 出差6天, 市内交通合计 600 > 80*6=480, 封顶为480
        let results = vec![
            make_match_result_from_invoice(make_invoice("inv1", 553.0, InvoiceCategory::Train)),
            make_match_result_from_invoice(make_invoice("inv2", 300.0, InvoiceCategory::CityTransport)),
            make_match_result_from_invoice(make_invoice("inv3", 300.0, InvoiceCategory::CityTransport)),
        ];
        let form = build_reimbursement_form(
            &results, "", "", "", "2025-08-04", "2025-08-09", 0, "其他人员",
        );
        assert_eq!(form.travel_days, 6);
        assert_eq!(form.city_transport_count, 2);
        assert!((form.city_transport_amount - 480.0).abs() < 0.01);
        // total = 553 + 480(封顶) + 600(伙食) = 1633
        assert!((form.total_amount - 1633.0).abs() < 0.01);
    }

    #[test]
    fn test_days_between() {
        assert_eq!(days_between("2025-08-04", "2025-08-15"), 12);
        assert_eq!(days_between("2025-01-01", "2025-01-01"), 1);
    }
}
