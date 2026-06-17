use crate::models::invoice::{Invoice, InvoiceCategory};
use crate::models::match_result::{MatchResult, MatchType};
use crate::models::payment::PaymentRecord;
use super::engine::MatchEngine;
use chrono::{Datelike, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMatchResult {
    pub matched: Vec<MatchResult>,
    pub unmatched_invoices: Vec<Invoice>,
    pub unmatched_payments: Vec<PaymentRecord>,
}

pub fn batch_match(
    invoices: &[Invoice],
    payments: &[PaymentRecord],
    tolerance: f64,
) -> BatchMatchResult {
    let engine = MatchEngine::new(tolerance);
    let mut matched = Vec::new();
    let mut unmatched_invoices = Vec::new();
    let mut used_payment_ids: Vec<String> = Vec::new();

    for invoice in invoices {
        let available_payments: Vec<PaymentRecord> = payments
            .iter()
            .filter(|p| !used_payment_ids.contains(&p.id))
            .cloned()
            .collect();

        let result = if invoice.category == InvoiceCategory::CityTransport
            && !invoice.itineraries.is_empty()
        {
            if invoice.itineraries.len() > 1 {
                match_itinerary_to_payments(invoice, &available_payments, tolerance)
                    .or_else(|| {
                        let time_filtered = filter_payments_by_itinerary_time(invoice, &available_payments);
                        engine.match_one_to_many(invoice, &time_filtered)
                    })
            } else {
                let time_filtered = filter_payments_by_itinerary_time(invoice, &available_payments);
                engine.match_one_to_many(invoice, &time_filtered)
            }
        } else {
            // 普通场景：一对一匹配
            engine.match_one_to_one(invoice, &available_payments)
        };

        if let Some(match_result) = result {
            for pid in &match_result.payment_ids {
                used_payment_ids.push(pid.clone());
            }
            matched.push(match_result);
        } else {
            unmatched_invoices.push(invoice.clone());
        }
    }

    let unmatched_payments: Vec<PaymentRecord> = payments
        .iter()
        .filter(|p| !used_payment_ids.contains(&p.id))
        .cloned()
        .collect();

    BatchMatchResult {
        matched,
        unmatched_invoices,
        unmatched_payments,
    }
}

/// 按行程时间范围过滤支付记录：支付时间必须在首条行程时间之后、且不超过12小时
fn filter_payments_by_itinerary_time(invoice: &Invoice, payments: &[PaymentRecord]) -> Vec<PaymentRecord> {
    let first_itin_time = invoice.itineraries.first()
        .and_then(|e| parse_datetime(&e.date_time));

    match first_itin_time {
        Some(it) => {
            let mut filtered: Vec<PaymentRecord> = payments.iter()
                .filter(|p| {
                    let pt = match parse_datetime(&p.transaction_time) {
                        Some(t) => t,
                        None => return false,
                    };
                    if pt < it { return false; }
                    let hours = (pt - it).num_hours();
                    hours <= 12
                })
                .cloned()
                .collect();
            filtered.sort_by(|a, b| {
                let ha = parse_datetime(&a.transaction_time)
                    .map(|t| (t - it).num_hours())
                    .unwrap_or(i64::MAX);
                let hb = parse_datetime(&b.transaction_time)
                    .map(|t| (t - it).num_hours())
                    .unwrap_or(i64::MAX);
                ha.cmp(&hb)
            });
            filtered
        }
        None => payments.to_vec(),
    }
}

/// 按行程单条目逐条匹配支付记录
/// 先宽容匹配确定真实商户，再锁定该商户重新匹配
fn match_itinerary_to_payments(
    invoice: &Invoice,
    payments: &[PaymentRecord],
    tolerance: f64,
) -> Option<MatchResult> {
    // === 第1轮：宽容匹配 ===
    // 优先匹配行程单服务商对应的商户，匹配不上再放宽
    struct Candidate {
        payment: PaymentRecord,
    }
    let entry_count = invoice.itineraries.len();
    let mut candidates: Vec<Option<Candidate>> = Vec::with_capacity(entry_count);
    candidates.resize_with(entry_count, || None);
    let mut used_ids: Vec<String> = Vec::new();

    // 从行程单确定服务商（如"天府通""滴滴出行"）
    let provider = invoice.itineraries.first()
        .map(|e| e.provider.to_lowercase())
        .filter(|p| !p.is_empty())
        .unwrap_or_default();

    for (idx, entry) in invoice.itineraries.iter().enumerate() {
        // 先找服务商对应商户的支付，匹配不上再放宽到所有商户
        let matched = if !provider.is_empty() {
            find_best_payment(entry, payments, &used_ids, Some(provider.as_str()), tolerance)
                .or_else(|| find_best_payment(entry, payments, &used_ids, None, tolerance))
        } else {
            find_best_payment(entry, payments, &used_ids, None, tolerance)
        };
        if let Some((pay, _)) = matched {
            used_ids.push(pay.id.clone());
            candidates[idx] = Some(Candidate { payment: pay });
        }
    }

    // 统计各商户出现次数，取出现最多的为真实商户
    let mut merchant_counts: HashMap<String, usize> = HashMap::new();
    for c in candidates.iter().flatten() {
        let key = if c.payment.merchant_name.is_empty() {
            "__unknown__".to_string()
        } else {
            c.payment.merchant_name.to_lowercase()
        };
        *merchant_counts.entry(key).or_default() += 1;
    }

    let real_merchant = merchant_counts.into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(name, _)| name)
        .unwrap_or_default();

    // === 第2轮：锁定真实商户，重新匹配 ===
    // 清空所有匹配，仅用真实商户的支付重新逐条匹配
    let mut matched_payments: Vec<PaymentRecord> = Vec::new();
    let mut final_used: Vec<String> = Vec::new();

    for entry in &invoice.itineraries {
        let matched = find_best_payment(
            entry,
            payments,
            &final_used,
            Some(&real_merchant),
            tolerance,
        );
        match matched {
            Some((pay, _)) => {
                final_used.push(pay.id.clone());
                matched_payments.push(pay);
            }
            None => return None,
        }
    }

    let total: f64 = matched_payments.iter().map(|p| p.amount).sum();
    let diff = (invoice.amount - total).abs();
    let payment_ids: Vec<String> = matched_payments.iter().map(|p| p.id.clone()).collect();

    Some(MatchResult {
        invoice_id: invoice.id.clone(),
        invoice: invoice.clone(),
        payment_ids,
        payments: matched_payments,
        match_type: MatchType::OneToMany,
        confidence: 1.0 - (diff / invoice.amount.max(0.01)),
        amount_diff: diff,
    })
}

/// 为单个行程条目找最佳匹配支付
/// merchant_filter: Some("xxx") 时只匹配该商户，None 时不限商户
/// 时间约束：支付时间不能早于行程时间，且不能晚于行程时间太久（48小时）
fn find_best_payment(
    entry: &crate::models::invoice::Itinerary,
    payments: &[PaymentRecord],
    used_ids: &[String],
    merchant_filter: Option<&str>,
    tolerance: f64,
) -> Option<(PaymentRecord, f64)> {
    let itin_time = parse_datetime(&entry.date_time);
    let mut exact_best: Option<(f64, f64, &PaymentRecord)> = None; // (amount_diff, hours_after, payment)
    let mut toll_best: Option<(f64, f64, &PaymentRecord)> = None;

    for pay in payments {
        if used_ids.contains(&pay.id) {
            continue;
        }
        if let Some(m) = merchant_filter {
            if !pay.merchant_name.to_lowercase().contains(m) {
                continue;
            }
        }

        let pay_time = parse_datetime(&pay.transaction_time);

        if let (Some(it), Some(pt)) = (&itin_time, &pay_time) {
            if pt < it {
                continue;
            }
            let hours_after = (*pt - *it).num_hours();
            if hours_after > 12 {
                continue;
            }
        }

        let diff = pay.amount - entry.amount;

        if entry.amount > 0.0 && pay.amount > 0.0 {
            let ratio = pay.amount / entry.amount;
            if ratio > 3.0 || ratio < 0.33 {
                continue;
            }
        }

        let hours_after = match (&itin_time, &pay_time) {
            (Some(it), Some(pt)) if pt >= it => (*pt - *it).num_hours().max(0) as f64,
            _ => 999.0,
        };

        if diff.abs() <= tolerance {
            let abs_diff = diff.abs();
            match exact_best {
                Some((best_diff, best_h, _))
                    if hours_after < best_h || ((hours_after - best_h).abs() < f64::EPSILON && abs_diff < best_diff) =>
                {
                    exact_best = Some((abs_diff, hours_after, pay));
                }
                None => exact_best = Some((abs_diff, hours_after, pay)),
                _ => {}
            }
            continue;
        }

        if diff > 0.0 && diff <= entry.amount {
            if is_same_day_or_next_morning(&entry.date_time, &pay.transaction_time) {
                match toll_best {
                    Some((best_diff, best_h, _))
                        if hours_after < best_h || ((hours_after - best_h).abs() < f64::EPSILON && diff < best_diff) =>
                    {
                        toll_best = Some((diff, hours_after, pay));
                    }
                    None => toll_best = Some((diff, hours_after, pay)),
                    _ => {}
                }
            }
        }
    }

    exact_best
        .map(|(d, _, p)| (p.clone(), d))
        .or_else(|| toll_best.map(|(d, _, p)| (p.clone(), d)))
}

/// 解析时间字符串为 NaiveDateTime
/// 支持: "YYYY-MM-DD HH:MM", "YYYY-MM-DD HH:MM:SS", "YYYY-MM-DD"
///       "YYYY/MM/DD ..."、无空格 "YYYY-MM-DDHH:MM"、无年份 "MM-DD HH:MM"
fn parse_datetime(time_str: &str) -> Option<NaiveDateTime> {
    // 去除尾部 ':'（行程 OCR 可能产出 "04-22 21:" 格式）
    let cleaned = time_str.trim().trim_end_matches(':').trim().to_string();

    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
        "%Y/%m/%d",
        "%Y-%m-%d%H:%M:%S",  // 无空格 如 "2026-04-2408:48:00"
        "%Y-%m-%d%H:%M",     // 无空格 如 "2026-04-2408:48"
    ];
    for fmt in formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&cleaned, fmt) {
            return Some(dt);
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(&cleaned, fmt) {
            return d.and_hms_opt(0, 0, 0);
        }
    }

    // Fallback: 无年份 MM-DD 格式，尝试拼接当年/去年
    // 检测 "04-25 08:48" 或 "04-22 21" 等
    if cleaned.len() >= 5
        && cleaned.as_bytes().get(2) == Some(&b'-')
        && cleaned[..2].chars().all(|c| c.is_ascii_digit())
        && cleaned[3..5].chars().all(|c| c.is_ascii_digit())
    {
        let current_year = chrono::Local::now().year();
        for year in [current_year, current_year - 1] {
            let with_year = format!("{}-{}", year, cleaned);
            for fmt in &formats {
                if let Ok(dt) = NaiveDateTime::parse_from_str(&with_year, fmt) {
                    return Some(dt);
                }
                if let Ok(d) = chrono::NaiveDate::parse_from_str(&with_year, fmt) {
                    return d.and_hms_opt(0, 0, 0);
                }
            }
        }
    }

    None
}

/// 判断支付时间是否在行程当天或次日凌晨
fn is_same_day_or_next_morning(itinerary_time: &str, payment_time: &str) -> bool {
    let itin_date = extract_date(itinerary_time);
    let pay_date = extract_date(payment_time);

    if itin_date.is_empty() || pay_date.is_empty() {
        // 无法解析时间，放宽限制允许匹配
        return true;
    }

    if let (Ok(d1), Ok(d2)) = (
        chrono::NaiveDate::parse_from_str(&itin_date, "%Y-%m-%d"),
        chrono::NaiveDate::parse_from_str(&pay_date, "%Y-%m-%d"),
    ) {
        let diff = (d2 - d1).num_days();
        return diff == 0 || diff == 1;
    }

    true
}

/// 从时间字符串中提取日期 "YYYY-MM-DD"
/// 支持: "YYYY-MM-DD HH:MM:SS" / "MM-DD HH:" / Excel序列号
fn extract_date(time_str: &str) -> String {
    // "2026-04-24 17:58:59" -> "2026-04-24"
    if time_str.len() >= 10 && time_str.as_bytes()[4] == b'-' {
        return time_str[..10].to_string();
    }
    // "04-22 21:" 或 "04-22" -> "2026-04-22"
    if time_str.len() >= 5 && time_str.as_bytes()[2] == b'-' {
        let mmdd = &time_str[..5];
        if mmdd.bytes().all(|c| c.is_ascii_digit() || c == b'-') {
            return format!("2026-{}", mmdd);
        }
    }
    // Excel 序列号 "46134.932" -> 日期
    if let Ok(serial) = time_str.parse::<f64>() {
        if serial > 40000.0 && serial < 55000.0 {
            let days_since_epoch = serial as i64 - 25569;
            let timestamp = days_since_epoch * 86400;
            if let Some(dt) = chrono::DateTime::from_timestamp(timestamp, 0).map(|dt| dt.naive_utc()) {
                return dt.format("%Y-%m-%d").to_string();
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{InvoiceCategory, InvoiceSource, Itinerary};
    use crate::models::match_result::MatchType;
    use crate::models::payment::PaymentSource;
    use chrono::NaiveDate;

    fn make_invoice(id: &str, amount: f64, category: InvoiceCategory) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "Test Seller".to_string(),
            item_name: "Test Item".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            category,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
        }
    }

    fn make_city_transport_invoice(id: &str, amount: f64) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "滴滴出行".to_string(),
            item_name: "市内交通".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![Itinerary {
                date_time: "2025-01-01 09:00".to_string(),
                provider: "滴滴".to_string(),
                pickup: "A站".to_string(),
                dropoff: "B站".to_string(),
                amount: 30.00,
            }],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
        }
    }

    fn make_payment(id: &str, amount: f64) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            transaction_id: format!("TX-{}", id),
            transaction_time: "2025-01-01 12:00".to_string(),
            amount,
            original_amount: amount,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: "Test Merchant".to_string(),
            source: PaymentSource::Wechat,
            category: "交通".to_string(),
            payment_method: String::new(),
        }
    }

    #[test]
    fn test_batch_match_mixed_invoices() {
        // 2张普通发票 + 1张打车发票
        let invoices = vec![
            make_invoice("inv1", 100.00, InvoiceCategory::Hotel),
            make_invoice("inv2", 50.00, InvoiceCategory::Meal),
            make_city_transport_invoice("inv3", 100.00),
        ];

        // 5笔支付
        let payments = vec![
            make_payment("p1", 100.00),  // 匹配 inv1 (一对一)
            make_payment("p2", 50.00),   // 匹配 inv2 (一对一)
            make_payment("p3", 30.00),   // 匹配 inv3 (一对多组合)
            make_payment("p4", 40.00),   // 匹配 inv3 (一对多组合)
            make_payment("p5", 30.50),   // 匹配 inv3 (一对多组合) → 30+40+30.50=100.50
        ];

        let result = batch_match(&invoices, &payments, 1.00);

        // 应该全部匹配成功
        assert_eq!(result.matched.len(), 3);
        assert_eq!(result.unmatched_invoices.len(), 0);
        assert_eq!(result.unmatched_payments.len(), 0);

        // 验证一对一匹配的发票
        let one_to_one_results: Vec<&MatchResult> = result
            .matched
            .iter()
            .filter(|r| matches!(r.match_type, MatchType::OneToOne))
            .collect();
        assert_eq!(one_to_one_results.len(), 2);

        // 验证一对多匹配的打车发票
        let one_to_many_results: Vec<&MatchResult> = result
            .matched
            .iter()
            .filter(|r| matches!(r.match_type, MatchType::OneToMany))
            .collect();
        assert_eq!(one_to_many_results.len(), 1);
        assert_eq!(one_to_many_results[0].invoice_id, "inv3");
        assert_eq!(one_to_many_results[0].payment_ids.len(), 3);

        let total: f64 = one_to_many_results[0].payments.iter().map(|p| p.amount).sum();
        assert!((total - 100.50).abs() < 0.01);
    }

    #[test]
    fn test_batch_match_no_match() {
        let invoices = vec![
            make_invoice("inv1", 500.00, InvoiceCategory::Hotel),
            make_invoice("inv2", 999.00, InvoiceCategory::Meal),
        ];

        let payments = vec![
            make_payment("p1", 10.00),
            make_payment("p2", 20.00),
        ];

        let result = batch_match(&invoices, &payments, 1.00);

        // 没有匹配
        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 2);
        assert_eq!(result.unmatched_payments.len(), 2);
    }

    #[test]
    fn test_batch_match_partial_match() {
        let invoices = vec![
            make_invoice("inv1", 100.00, InvoiceCategory::Hotel),
            make_invoice("inv2", 999.00, InvoiceCategory::Meal),
        ];

        let payments = vec![
            make_payment("p1", 100.00), // 匹配 inv1
            make_payment("p2", 20.00),  // 无匹配
        ];

        let result = batch_match(&invoices, &payments, 1.00);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.unmatched_invoices.len(), 1);
        assert_eq!(result.unmatched_payments.len(), 1);
        assert_eq!(result.unmatched_invoices[0].id, "inv2");
        assert_eq!(result.unmatched_payments[0].id, "p2");
    }

    #[test]
    fn test_batch_match_city_transport_without_itineraries_uses_one_to_one() {
        // 打车类别但没有行程，应走一对一匹配
        let mut invoice = make_invoice("inv1", 100.00, InvoiceCategory::CityTransport);
        invoice.itineraries = vec![]; // 无行程

        let payments = vec![
            make_payment("p1", 30.00),
            make_payment("p2", 70.00),  // 30+70=100 但不应该一对多
            make_payment("p3", 100.00), // 精确匹配
        ];

        let result = batch_match(&[invoice], &payments, 1.00);

        assert_eq!(result.matched.len(), 1);
        assert!(matches!(result.matched[0].match_type, MatchType::OneToOne));
        assert_eq!(result.matched[0].payment_ids, vec!["p3".to_string()]);
    }

    #[test]
    fn test_batch_match_empty_inputs() {
        let result = batch_match(&[], &[], 1.00);
        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 0);
        assert_eq!(result.unmatched_payments.len(), 0);

        let invoices = vec![make_invoice("inv1", 100.00, InvoiceCategory::Other)];
        let result = batch_match(&invoices, &[], 1.00);
        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 1);

        let payments = vec![make_payment("p1", 100.00)];
        let result = batch_match(&[], &payments, 1.00);
        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_payments.len(), 1);
    }

    fn make_payment_at(id: &str, amount: f64, time: &str) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            transaction_id: format!("TX-{}", id),
            transaction_time: time.to_string(),
            amount,
            original_amount: amount,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: "Test Merchant".to_string(),
            source: PaymentSource::Wechat,
            category: "交通".to_string(),
            payment_method: String::new(),
        }
    }

    fn make_itinerary_invoice(id: &str, amount: f64, itin_time: &str, itin_amount: f64) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "滴滴出行".to_string(),
            item_name: "市内交通".to_string(),
            date: NaiveDate::parse_from_str(&itin_time[..10], "%Y-%m-%d").unwrap(),
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![Itinerary {
                date_time: itin_time.to_string(),
                provider: "滴滴".to_string(),
                pickup: "A站".to_string(),
                dropoff: "B站".to_string(),
                amount: itin_amount,
            }],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
        }
    }

    #[test]
    fn test_itinerary_rejects_payment_before_trip() {
        let invoice = make_itinerary_invoice("inv1", 30.00, "2025-01-15 09:00", 30.00);
        let payments = vec![
            make_payment_at("p1", 30.00, "2025-01-15 08:00"),
            make_payment_at("p2", 30.00, "2025-01-15 09:30"),
        ];

        let result = batch_match(&[invoice], &payments, 1.00);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].payment_ids, vec!["p2".to_string()]);
        assert_eq!(result.unmatched_payments.len(), 1);
        assert_eq!(result.unmatched_payments[0].id, "p1");
    }

    #[test]
    fn test_itinerary_rejects_payment_too_far_after_trip() {
        let invoice = make_itinerary_invoice("inv1", 30.00, "2025-01-15 09:00", 30.00);
        let payments = vec![
            make_payment_at("p1", 30.00, "2025-01-16 10:00"),
        ];

        let result = batch_match(&[invoice], &payments, 1.00);

        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 1);
    }

    #[test]
    fn test_itinerary_prefers_closer_time_match() {
        let invoice = make_itinerary_invoice("inv1", 30.00, "2025-01-15 09:00", 30.00);
        let payments = vec![
            make_payment_at("p1", 30.00, "2025-01-15 20:00"),
            make_payment_at("p2", 30.00, "2025-01-15 09:15"),
        ];

        let result = batch_match(&[invoice], &payments, 1.00);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].payment_ids, vec!["p2".to_string()]);
    }

    #[test]
    fn test_itinerary_allows_payment_same_day_after_trip() {
        let invoice = make_itinerary_invoice("inv1", 30.00, "2025-01-15 09:00", 30.00);
        let payments = vec![
            make_payment_at("p1", 30.00, "2025-01-15 09:05"),
        ];

        let result = batch_match(&[invoice], &payments, 1.00);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].payment_ids, vec!["p1".to_string()]);
    }

    #[test]
    fn test_itinerary_allows_payment_within_12h() {
        let invoice = make_itinerary_invoice("inv1", 30.00, "2025-01-15 09:00", 30.00);
        let payments = vec![
            make_payment_at("p1", 30.00, "2025-01-15 20:00"),
        ];

        let result = batch_match(&[invoice], &payments, 1.00);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].payment_ids, vec!["p1".to_string()]);
    }

    #[test]
    fn test_itinerary_rejects_payment_beyond_12h() {
        let invoice = make_itinerary_invoice("inv1", 30.00, "2025-01-15 09:00", 30.00);
        let payments = vec![
            make_payment_at("p1", 30.00, "2025-01-16 08:00"),
        ];

        let result = batch_match(&[invoice], &payments, 1.00);

        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 1);
    }
}
