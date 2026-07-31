use crate::models::invoice::{Invoice, InvoiceCategory};
use crate::models::match_result::{MatchResult, MatchType, ItineraryPaymentPair};
use crate::models::payment::PaymentRecord;
use super::engine::{MatchEngine, filter_payments_by_date_direction, parse_payment_date};
use super::strategy_selector::{MatchingStrategy, StrategySelector};
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
    // 按交易时间升序排序，消除文件读取顺序偏差（微信/支付宝混合时不再按导入顺序）
    let mut payments_sorted: Vec<PaymentRecord> = payments.to_vec();
    sort_payments_by_time(&mut payments_sorted);
    // 过滤退款交易（退款金额 > 0 或实际支付金额 <= 0）
    let payments_sorted: Vec<PaymentRecord> = payments_sorted
        .into_iter()
        .filter(|p| !p.is_refund())
        .collect();
    let payments = &payments_sorted[..];

    // 分离 Toll 发票和其他发票
    let toll_invoices: Vec<Invoice> = invoices.iter()
        .filter(|inv| inv.category == InvoiceCategory::Toll)
        .cloned()
        .collect();
    let non_toll_invoices: Vec<Invoice> = invoices.iter()
        .filter(|inv| inv.category != InvoiceCategory::Toll)
        .cloned()
        .collect();

    let mut matched = Vec::new();
    let mut unmatched_invoices = Vec::new();
    let mut used_payment_ids: HashSet<String> = HashSet::new();

    // === 第一阶段：高速费单独匹配（最先，避免支付被行程占据）===
    // match_one_to_one 内部已统一用 toll_travel_time 匹配（见 engine.rs）
    let mut pending_tolls: Vec<Invoice> = Vec::new();  // 单独匹配失败，待关联行程
    for toll in &toll_invoices {
        let available: Vec<PaymentRecord> = payments
            .iter()
            .filter(|p| !used_payment_ids.contains(&p.id))
            .cloned()
            .collect();
        if let Some(mr) = engine.match_one_to_one(toll, &available) {
            for pid in &mr.payment_ids {
                used_payment_ids.insert(pid.clone());
            }
            matched.push(mr);
        } else {
            pending_tolls.push(toll.clone());
        }
    }

    // === 第二阶段：高速费关联行程组合匹配 ===
    // 待关联 Toll 按通行时间找最近的 CityTransport，用组合金额匹配
    let city_transport_invoices: Vec<&Invoice> = non_toll_invoices.iter()
        .filter(|inv| inv.category == InvoiceCategory::CityTransport && !inv.itineraries.is_empty())
        .collect();

    // toll_id -> city_transport_id 关联
    let mut toll_links: HashMap<String, String> = HashMap::new();
    for toll in &pending_tolls {
        let toll_time = toll.toll_travel_time
            .or_else(|| toll.date.and_hms_opt(0, 0, 0));
        if let Some(tt) = toll_time {
            let best = city_transport_invoices.iter()
                .filter(|ct| !toll_links.values().any(|linked_id| linked_id.as_str() == ct.id.as_str()))
                .min_by_key(|ct| {
                    ct.itineraries.first()
                        .and_then(|e| parse_datetime(&e.date_time))
                        .map(|it| (it - tt).num_seconds().unsigned_abs())
                        .unwrap_or(u64::MAX)
                });
            if let Some(ct) = best {
                toll_links.insert(toll.id.clone(), ct.id.clone());
            }
        }
    }

    // 记录已被高速费组合匹配占用的行程ID
    let mut trip_matched_by_toll: Vec<String> = Vec::new();

    // 对每个有关联 Toll 的行程，用组合金额匹配
    for ct in &city_transport_invoices {
        let linked_tolls: Vec<&Invoice> = pending_tolls.iter()
            .filter(|t| toll_links.get(&t.id).map(|id| id.as_str()) == Some(ct.id.as_str()))
            .collect();
        if linked_tolls.is_empty() {
            continue;
        }

        let linked_toll_amount: f64 = linked_tolls.iter().map(|t| t.amount).sum();
        let combined_amount = ct.amount + linked_toll_amount;

        let available: Vec<PaymentRecord> = payments
            .iter()
            .filter(|p| !used_payment_ids.contains(&p.id))
            .cloned()
            .collect();

        // 用组合金额构造临时发票
        let mut invoice_for_match = (*ct).clone();
        invoice_for_match.amount = combined_amount;

        let result = if ct.itineraries.len() > 1 {
            match_itinerary_to_payments(&invoice_for_match, &available, tolerance)
                .or_else(|| {
                    let time_filtered = filter_payments_by_itinerary_time(&invoice_for_match, &available);
                    engine.match_one_to_many(&invoice_for_match, &time_filtered)
                })
        } else {
            let time_filtered = filter_payments_by_itinerary_time(&invoice_for_match, &available);
            engine.match_one_to_many(&invoice_for_match, &time_filtered)
        };

        if let Some(match_result) = result {
            let payment_ids = match_result.payment_ids.clone();
            let matched_payments = match_result.payments.clone();
            let trip_confidence = match_result.confidence;
            for pid in &payment_ids {
                used_payment_ids.insert(pid.clone());
            }

            // 行程发票 MatchResult（用原始金额）
            let total: f64 = matched_payments.iter().map(|p| p.amount).sum();
            let trip_result = MatchResult {
                invoice_id: ct.id.clone(),
                invoice: (*ct).clone(),
                payment_ids: payment_ids.clone(),
                payments: matched_payments.clone(),
                match_type: match_result.match_type,
                confidence: trip_confidence,
                amount_diff: (ct.amount - total).abs(),
                itinerary_payment_pairs: match_result.itinerary_payment_pairs,
                shared_payment_ids: vec![],
                shared_from_invoice_id: None,
            };
            matched.push(trip_result);
            trip_matched_by_toll.push(ct.id.clone());

            // 高速费共享 MatchResult（永远 OneToOne）
            for toll in &linked_tolls {
                let toll_match = MatchResult {
                    invoice_id: toll.id.clone(),
                    invoice: (*toll).clone(),
                    payment_ids: payment_ids.clone(),
                    payments: matched_payments.clone(),
                    match_type: MatchType::OneToOne,
                    confidence: trip_confidence,
                    amount_diff: 0.0,
                    itinerary_payment_pairs: vec![],
                    shared_payment_ids: payment_ids.clone(),
                    shared_from_invoice_id: Some(ct.id.clone()),
                };
                matched.push(toll_match);
            }
        } else {
            // 组合匹配失败：解除关联，Toll 尝试其他行程
            for toll in &linked_tolls {
                toll_links.remove(&toll.id);
                let toll_time = toll.toll_travel_time
                    .or_else(|| toll.date.and_hms_opt(0, 0, 0));
                if let Some(tt) = toll_time {
                    let best = city_transport_invoices.iter()
                        .filter(|other| other.id != ct.id)
                        .filter(|other| !toll_links.values().any(|linked_id| linked_id.as_str() == other.id.as_str()))
                        .filter(|other| !trip_matched_by_toll.contains(&other.id))
                        .min_by_key(|other| {
                            other.itineraries.first()
                                .and_then(|e| parse_datetime(&e.date_time))
                                .map(|it| (it - tt).num_seconds().unsigned_abs())
                                .unwrap_or(u64::MAX)
                        });
                    if let Some(other) = best {
                        toll_links.insert(toll.id.clone(), other.id.clone());
                    }
                }
            }
        }
    }

    // 仍未匹配的 Toll → unmatched
    let matched_toll_ids: Vec<String> = matched.iter()
        .filter(|m| m.invoice.category == InvoiceCategory::Toll)
        .map(|m| m.invoice.id.clone())
        .collect();
    for toll in &pending_tolls {
        if !matched_toll_ids.contains(&toll.id) {
            unmatched_invoices.push(toll.clone());
        }
    }

    // === 第三阶段：剩余行程单独匹配（按金额降序，改善贪心匹配质量） ===
    let mut sorted_non_toll: Vec<&Invoice> = non_toll_invoices.iter()
        .filter(|inv| !trip_matched_by_toll.contains(&inv.id))
        .collect();
    sorted_non_toll.sort_by(|a, b| b.amount.partial_cmp(&a.amount).unwrap_or(std::cmp::Ordering::Equal));

    for invoice in sorted_non_toll {
        let available: Vec<PaymentRecord> = payments
            .iter()
            .filter(|p| !used_payment_ids.contains(&p.id))
            .cloned()
            .collect();

        let result = if invoice.category == InvoiceCategory::CityTransport
            && !invoice.itineraries.is_empty()
        {
            if invoice.itineraries.len() > 1 {
                match_itinerary_to_payments(invoice, &available, tolerance)
                    .or_else(|| {
                        let time_filtered = filter_payments_by_itinerary_time(invoice, &available);
                        engine.match_one_to_many(invoice, &time_filtered)
                    })
            } else {
                let time_filtered = filter_payments_by_itinerary_time(invoice, &available);
                engine.match_one_to_many(invoice, &time_filtered)
            }
        } else if invoice.category == InvoiceCategory::Insurance {
            // 保险费特殊匹配：利用同批次机票的支付时间和出行时间约束
            // 策略：精确时间戳匹配 → 机票窗口内匹配 → 出行时间上界 → 硬过滤回退
            let flight_windows = collect_flight_windows(&matched, invoices);
            match_insurance_invoice(&engine, invoice, &available, &flight_windows, tolerance)
        } else {
            // 非行程类发票："先付费后开票"，排除支付时间晚于开票日期的记录
            // 保险费/退改签等无交易时间锚点的发票只有开票时间，延迟开票时
            // .abs() 对称匹配会错配到开票后的无关支付，这里做硬过滤
            let available = filter_payments_by_date_direction(&available, invoice.date);
            let strategy = StrategySelector::select(invoice, available.len());
            match strategy {
                MatchingStrategy::AmountWithMerchant if !invoice.seller_name.is_empty() => {
                    // 优先选同商户的支付
                    let merchant_lower = invoice.seller_name.to_lowercase();
                    let merchant_filtered: Vec<PaymentRecord> = available.iter()
                        .filter(|p| {
                            let m = p.merchant_name.to_lowercase();
                            m.contains(&merchant_lower) || merchant_lower.contains(&m)
                        })
                        .cloned()
                        .collect();
                    if !merchant_filtered.is_empty() {
                        engine.match_one_to_one(invoice, &merchant_filtered)
                            .or_else(|| engine.match_one_to_one(invoice, &available))
                    } else {
                        engine.match_one_to_one(invoice, &available)
                    }
                }
                _ => engine.match_one_to_one(invoice, &available),
            }
        };

        if let Some(match_result) = result {
            for pid in &match_result.payment_ids {
                used_payment_ids.insert(pid.clone());
            }
            matched.push(match_result);
        } else {
            // 行程匹配失败：尝试 toll_best 容差（支付 > 行程金额，差额可能是未开票高速费）
            if invoice.category == InvoiceCategory::CityTransport && !invoice.itineraries.is_empty() {
                if let Some(toll_match) = match_trip_with_toll_tolerance(invoice, &available, tolerance) {
                    for pid in &toll_match.payment_ids {
                        used_payment_ids.insert(pid.clone());
                    }
                    matched.push(toll_match);
                } else {
                    unmatched_invoices.push(invoice.clone());
                }
            } else {
                unmatched_invoices.push(invoice.clone());
            }
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

/// 机票时间窗口：用于保险费匹配时约束支付时间范围
#[derive(Debug, Clone)]
struct FlightWindow {
    payment_time: String,       // 已匹配机票的支付时间（空串表示机票未匹配到支付）
    travel_date: NaiveDate,     // 机票出行日期
}

/// 收集同批次内所有机票发票的时间窗口
/// - 已匹配的机票：包含支付时间 + 出行时间
/// - 未匹配但有出行日期的机票：只有出行时间（支付时间为空）
fn collect_flight_windows(matched: &[MatchResult], all_invoices: &[Invoice]) -> Vec<FlightWindow> {
    let mut windows = Vec::new();

    for m in matched {
        if m.invoice.category == InvoiceCategory::Flight {
            if let Some(travel_date) = m.invoice.travel_date {
                if let Some(first_payment) = m.payments.first() {
                    windows.push(FlightWindow {
                        payment_time: first_payment.transaction_time.clone(),
                        travel_date,
                    });
                } else {
                    windows.push(FlightWindow {
                        payment_time: String::new(),
                        travel_date,
                    });
                }
            }
        }
    }

    // 补充未匹配但有出行日期的机票
    for inv in all_invoices {
        if inv.category == InvoiceCategory::Flight {
            if let Some(travel_date) = inv.travel_date {
                let already_in = windows.iter().any(|w| w.travel_date == travel_date);
                if !already_in {
                    windows.push(FlightWindow {
                        payment_time: String::new(),
                        travel_date,
                    });
                }
            }
        }
    }

    windows
}

/// 保险费发票 4 级匹配策略：
/// 1. 精确时间戳匹配：支付时间与某张机票支付时间相同（同一秒）
/// 2. 机票窗口内匹配：支付落在任意机票的 [支付时间, 出行时间] 区间内
/// 3. 出行时间上界：支付日期 ≤ 最晚机票出行日期（无支付时间信息时）
/// 4. 硬过滤回退：支付日期 ≤ 开票日期（当前通用逻辑）
fn match_insurance_invoice(
    engine: &MatchEngine,
    invoice: &Invoice,
    available: &[PaymentRecord],
    flight_windows: &[FlightWindow],
    _tolerance: f64,
) -> Option<MatchResult> {
    // 阶段 1：精确时间戳匹配（保险和机票同一秒支付）
    for window in flight_windows {
        if window.payment_time.is_empty() {
            continue;
        }
        let exact: Vec<PaymentRecord> = available
            .iter()
            .filter(|p| p.transaction_time == window.payment_time)
            .cloned()
            .collect();
        if !exact.is_empty() {
            if let Some(r) = engine.match_one_to_one(invoice, &exact) {
                return Some(r);
            }
        }
    }

    // 阶段 2：在任意机票的 [支付时间, 出行时间] 窗口内匹配
    // 同时仍然约束支付 ≤ 开票日期（作为基线安全网）
    if !flight_windows.is_empty() {
        let window_filtered: Vec<PaymentRecord> = available
            .iter()
            .filter(|p| {
                let pd = parse_payment_date(&p.transaction_time);
                pd.map_or(true, |pd| {
                    // 基线：支付日期 ≤ 开票日期
                    if pd > invoice.date {
                        return false;
                    }
                    // 至少在一个机票窗口内
                    flight_windows.iter().any(|w| {
                        let wd = parse_payment_date(&w.payment_time);
                        let lower_ok = wd.map_or(true, |wd| pd >= wd);
                        let upper_ok = pd <= w.travel_date;
                        lower_ok && upper_ok
                    })
                })
            })
            .cloned()
            .collect();
        if !window_filtered.is_empty() {
            if let Some(r) = engine.match_one_to_one(invoice, &window_filtered) {
                return Some(r);
            }
        }
    }

    // 阶段 3：上界 = 最晚机票出行日期（无支付时间信息时的回退）
    if let Some(max_travel) = flight_windows.iter().map(|w| w.travel_date).max() {
        let bounded = filter_payments_by_date_direction(available, max_travel);
        if !bounded.is_empty() {
            if let Some(r) = engine.match_one_to_one(invoice, &bounded) {
                return Some(r);
            }
        }
    }

    // 阶段 4：回退到硬过滤（支付 ≤ 开票日期）
    let fallback = filter_payments_by_date_direction(available, invoice.date);
    engine.match_one_to_one(invoice, &fallback)
}

/// toll_best 容差匹配：行程金额 < 支付金额，差额可能是未开票的高速费。
/// 复用 find_best_payment 的 toll_best 逻辑：差额 <= 行程金额且时间在同一天或次日凌晨。
/// 仅用于 CityTransport 单行程场景的回退匹配。
fn match_trip_with_toll_tolerance(
    invoice: &Invoice,
    payments: &[PaymentRecord],
    _tolerance: f64,
) -> Option<MatchResult> {
    let entry = invoice.itineraries.first()?;
    let itin_time = parse_datetime(&entry.date_time)?;

    let mut best: Option<(f64, &PaymentRecord)> = None;
    for pay in payments {
        let pay_time = parse_datetime(&pay.transaction_time);
        if let Some(pt) = &pay_time {
            if pt < &itin_time {
                continue;
            }
        }
        // toll_best 条件：支付 > 行程金额，差额 <= 行程金额（支付不超过2倍）
        let diff = pay.amount - entry.amount;
        if diff > 0.0 && diff <= entry.amount {
            if is_same_day_or_next_morning(&entry.date_time, &pay.transaction_time) {
                match best {
                    Some((best_diff, _)) if diff < best_diff => best = Some((diff, pay)),
                    None => best = Some((diff, pay)),
                    _ => {}
                }
            }
        }
    }

    best.map(|(diff, pay)| MatchResult {
        invoice_id: invoice.id.clone(),
        invoice: invoice.clone(),
        payment_ids: vec![pay.id.clone()],
        payments: vec![pay.clone()],
        match_type: MatchType::OneToOne,
        confidence: 1.0 - (diff / invoice.amount.max(0.01)),
        amount_diff: diff,
        itinerary_payment_pairs: vec![],
        shared_payment_ids: vec![],
        shared_from_invoice_id: None,
    })
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
    // 行程-支付显式配对：matched_payments 按行程顺序逐条匹配，故下标即行程索引
    let itinerary_payment_pairs: Vec<ItineraryPaymentPair> = matched_payments
        .iter()
        .enumerate()
        .map(|(idx, p)| ItineraryPaymentPair {
            itinerary_index: idx,
            payment_id: p.id.clone(),
        })
        .collect();

    Some(MatchResult {
        invoice_id: invoice.id.clone(),
        invoice: invoice.clone(),
        payment_ids,
        payments: matched_payments,
        match_type: MatchType::OneToMany,
        confidence: 1.0 - (diff / invoice.amount.max(0.01)),
        amount_diff: diff,
        itinerary_payment_pairs,
        shared_payment_ids: vec![],
        shared_from_invoice_id: None,
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

/// 按交易时间升序稳定排序支付记录；时间无法解析的记录排在最后。
/// 用于在 batch_match 入口消除"按文件读取顺序"的偏差，使后续匹配
/// 遍历顺序与时间一致。
fn sort_payments_by_time(payments: &mut [PaymentRecord]) {
    payments.sort_by(|a, b| {
        let ta = parse_datetime(&a.transaction_time);
        let tb = parse_datetime(&b.transaction_time);
        match (ta, tb) {
            (Some(ta), Some(tb)) => ta.cmp(&tb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
}

/// 解析时间字符串为 NaiveDateTime
/// 支持: "YYYY-MM-DD HH:MM", "YYYY-MM-DD HH:MM:SS", "YYYY-MM-DD"
///       "YYYY/MM/DD ..."、无空格 "YYYY-MM-DDHH:MM"、无年份 "MM-DD HH:MM"
fn parse_datetime(time_str: &str) -> Option<NaiveDateTime> {
    crate::parser::datetime_util::parse_datetime(time_str)
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
    crate::parser::datetime_util::extract_date(time_str).unwrap_or_default()
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
            travel_date: None,
            category,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
                        toll_travel_time: None,
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
            travel_date: None,
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![Itinerary {
                date_time: "2025-01-01 09:00".to_string(),
                provider: "滴滴".to_string(),
                pickup: "A站".to_string(),
                dropoff: "B站".to_string(),
                amount: 30.00,
                incomplete_fields: vec![],
            }],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
                        toll_travel_time: None,
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

    fn make_invoice_at(id: &str, amount: f64, category: InvoiceCategory, date: &str) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: String::new(),
            amount,
            seller_name: "Test Seller".to_string(),
            item_name: "Test Item".to_string(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            travel_date: None,
            category,
            source: crate::models::invoice::InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: None,
        }
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
            travel_date: None,
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![Itinerary {
                date_time: itin_time.to_string(),
                provider: "滴滴".to_string(),
                pickup: "A站".to_string(),
                dropoff: "B站".to_string(),
                amount: itin_amount,
                incomplete_fields: vec![],
            }],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
                        toll_travel_time: None,
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

    #[test]
    fn test_itinerary_payment_pairs_populated_for_multi_itinerary() {
        // 2条行程的市内交通发票：行程1(09:00,30元) 行程2(14:00,40元)
        let mut invoice = make_city_transport_invoice("inv1", 70.00);
        invoice.itineraries = vec![
            Itinerary { date_time: "2025-01-15 09:00".to_string(), provider: "滴滴".to_string(), pickup: "A".to_string(), dropoff: "B".to_string(), amount: 30.00, incomplete_fields: vec![] },
            Itinerary { date_time: "2025-01-15 14:00".to_string(), provider: "滴滴".to_string(), pickup: "C".to_string(), dropoff: "D".to_string(), amount: 40.00, incomplete_fields: vec![] },
        ];
        // 支付顺序故意打乱：p1 对应行程2，p2 对应行程1
        let payments = vec![
            make_payment_at("p1", 40.00, "2025-01-15 14:05"),
            make_payment_at("p2", 30.00, "2025-01-15 09:05"),
        ];

        let result = batch_match(&[invoice], &payments, 1.00);
        assert_eq!(result.matched.len(), 1);
        let m = &result.matched[0];
        // pairs 应显式记录行程-支付配对，且按行程顺序
        assert_eq!(m.itinerary_payment_pairs.len(), 2);
        assert_eq!(m.itinerary_payment_pairs[0].itinerary_index, 0);
        assert_eq!(m.itinerary_payment_pairs[0].payment_id, "p2");
        assert_eq!(m.itinerary_payment_pairs[1].itinerary_index, 1);
        assert_eq!(m.itinerary_payment_pairs[1].payment_id, "p1");
    }

    #[test]
    fn test_sort_payments_by_time() {
        let mut payments = vec![
            make_payment_at("p1", 30.0, "2025-01-15 20:00"),
            make_payment_at("p2", 30.0, "2025-01-15 09:00"),
            make_payment_at("p3", 30.0, "2025-01-14 08:00"),
        ];
        sort_payments_by_time(&mut payments);
        assert_eq!(payments[0].id, "p3");
        assert_eq!(payments[1].id, "p2");
        assert_eq!(payments[2].id, "p1");
    }

    #[test]
    fn test_sort_payments_by_time_stable() {
        // 相同时间应保持原顺序（稳定排序）
        let mut payments = vec![
            make_payment_at("a", 30.0, "2025-01-15 09:00"),
            make_payment_at("b", 30.0, "2025-01-15 09:00"),
            make_payment_at("c", 30.0, "2025-01-15 09:00"),
        ];
        sort_payments_by_time(&mut payments);
        assert_eq!(payments[0].id, "a");
        assert_eq!(payments[1].id, "b");
        assert_eq!(payments[2].id, "c");
    }

    fn make_toll_invoice(id: &str, amount: f64, travel_time: &str) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: "ETC".to_string(),
            item_name: "通行费".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            travel_date: None,
            category: InvoiceCategory::Toll,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: format!("XX站入 XX站出 {}", travel_time),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: chrono::NaiveDateTime::parse_from_str(
                travel_time, "%Y-%m-%d %H:%M:%S"
            ).ok(),
        }
    }

    #[test]
    fn test_toll_auto_links_to_nearest_city_transport() {
        // 行程发票 50元，高速费 10元，支付 60元
        let mut invoice = make_city_transport_invoice("inv1", 50.00);
        invoice.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
            incomplete_fields: vec![],
        }];
        let toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        let payment = make_payment_at("p1", 60.00, "2025-01-15 09:35");

        let result = batch_match(&[invoice, toll], &[payment], 1.00);

        // 两张发票都应匹配成功，共用同一笔支付
        assert_eq!(result.matched.len(), 2);
        assert_eq!(result.unmatched_invoices.len(), 0);
        assert_eq!(result.unmatched_payments.len(), 0);

        // 行程发票 MatchResult
        let trip_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::CityTransport).unwrap();
        assert_eq!(trip_match.payment_ids, vec!["p1".to_string()]);
        assert!(trip_match.shared_payment_ids.is_empty());
        assert!(trip_match.shared_from_invoice_id.is_none());

        // 高速费 MatchResult
        let toll_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::Toll).unwrap();
        assert_eq!(toll_match.payment_ids, vec!["p1".to_string()]);
        assert_eq!(toll_match.shared_payment_ids, vec!["p1".to_string()]);
        assert_eq!(toll_match.shared_from_invoice_id, Some("inv1".to_string()));
    }

    #[test]
    fn test_toll_no_city_transport_independent_match() {
        // 没有行程发票，但高速费有单独支付，应单独匹配成功
        let toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        let payment = make_payment_at("p1", 10.00, "2025-01-15 09:35");

        let result = batch_match(&[toll], &[payment], 1.00);

        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].invoice.id, "toll1");
        assert!(result.matched[0].shared_from_invoice_id.is_none());
    }

    #[test]
    fn test_toll_combination_amount_matches_payment() {
        // 行程 50 + 高速费 10 = 60，支付 60元
        let mut invoice = make_city_transport_invoice("inv1", 50.00);
        invoice.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
            incomplete_fields: vec![],
        }];
        let toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        // 支付恰好 60 元（行程+高速费组合）
        let payment = make_payment_at("p1", 60.00, "2025-01-15 09:35");

        let result = batch_match(&[invoice, toll], &[payment], 1.00);

        assert_eq!(result.matched.len(), 2);
        let trip_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::CityTransport).unwrap();
        // 行程发票 50 元，支付 60 元，差额 10 元（高速费部分）
        assert!((trip_match.amount_diff - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_toll_falls_to_next_trip_if_first_fails() {
        // 两条行程：行程1(50元,09:00) 行程2(40元,14:00)
        // 高速费 20元，通行时间 14:30（更近行程2）
        // 支付1: 60元（行程1+高速费=70 不匹配） 支付2: 60元（行程2+高速费=60 匹配）
        let mut inv1 = make_city_transport_invoice("inv1", 50.00);
        inv1.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
            incomplete_fields: vec![],
        }];
        let mut inv2 = make_city_transport_invoice("inv2", 40.00);
        inv2.itineraries = vec![Itinerary {
            date_time: "2025-01-15 14:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "C".to_string(),
            dropoff: "D".to_string(),
            amount: 40.00,
            incomplete_fields: vec![],
        }];
        let toll = make_toll_invoice("toll1", 20.00, "2025-01-15 14:30:00");
        let payments = vec![
            make_payment_at("p1", 60.00, "2025-01-15 09:05"),  // 行程1时间附近，但50+20=70≠60
            make_payment_at("p2", 60.00, "2025-01-15 14:05"),  // 行程2时间附近，40+20=60 匹配
        ];

        let result = batch_match(&[inv1, inv2, toll], &payments, 1.00);

        // 高速费应关联到行程2（时间更近且金额组合匹配）
        let toll_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::Toll).unwrap();
        assert_eq!(toll_match.shared_from_invoice_id, Some("inv2".to_string()));
        assert_eq!(toll_match.payment_ids, vec!["p2".to_string()]);
    }

    #[test]
    fn test_toll_relinks_after_first_trip_fails() {
        // 反例场景：Toll 通行时间更近 inv1，但 inv1 组合金额不匹配，
        // 应解除关联并重新关联到 inv2（组合金额匹配）
        let mut inv1 = make_city_transport_invoice("inv1", 50.00);
        inv1.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
            incomplete_fields: vec![],
        }];
        let mut inv2 = make_city_transport_invoice("inv2", 40.00);
        inv2.itineraries = vec![Itinerary {
            date_time: "2025-01-15 14:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "C".to_string(),
            dropoff: "D".to_string(),
            amount: 40.00,
            incomplete_fields: vec![],
        }];
        // Toll 通行时间 09:15，更近 inv1（09:00）而非 inv2（14:00）
        let toll = make_toll_invoice("toll1", 20.00, "2025-01-15 09:15:00");
        let payments = vec![
            make_payment_at("p1", 50.00, "2025-01-15 09:10"),  // 仅匹配 inv1 单独金额，但 inv1+Toll=70≠50
            make_payment_at("p2", 60.00, "2025-01-15 14:05"),  // inv2+Toll=40+20=60 匹配
        ];

        let result = batch_match(&[inv1, inv2, toll], &payments, 1.00);

        // inv1 单独匹配 p1（50元）
        let inv1_match = result.matched.iter().find(|m| m.invoice.id == "inv1");
        assert!(inv1_match.is_some(), "inv1 应单独匹配 p1");
        assert_eq!(inv1_match.unwrap().payment_ids, vec!["p1".to_string()]);

        // Toll 应重新关联到 inv2，匹配 p2
        let toll_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::Toll);
        assert!(toll_match.is_some(), "Toll 应重新关联到 inv2 并匹配");
        assert_eq!(toll_match.unwrap().shared_from_invoice_id, Some("inv2".to_string()));
        assert_eq!(toll_match.unwrap().payment_ids, vec!["p2".to_string()]);
    }

    #[test]
    fn test_toll_independent_match_before_trip() {
        // 高速费有单独支付，应先单独匹配，不关联行程
        let mut inv = make_city_transport_invoice("inv1", 50.00);
        inv.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
            incomplete_fields: vec![],
        }];
        let toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        let payments = vec![
            make_payment_at("p_toll", 10.00, "2025-01-15 09:35"),  // 高速费单独支付
            make_payment_at("p_trip", 50.00, "2025-01-15 09:10"),  // 行程单独支付
        ];

        let result = batch_match(&[inv, toll], &payments, 1.00);

        // 两张发票都匹配成功，各自匹配自己的支付
        assert_eq!(result.matched.len(), 2);
        assert_eq!(result.unmatched_payments.len(), 0);

        let toll_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::Toll).unwrap();
        // 高速费单独匹配，不共享
        assert!(toll_match.shared_from_invoice_id.is_none());
        assert_eq!(toll_match.payment_ids, vec!["p_toll".to_string()]);
        // 高速费永远一对一
        assert!(matches!(toll_match.match_type, MatchType::OneToOne));
    }

    #[test]
    fn test_toll_combination_match_when_independent_fails() {
        // 高速费无单独支付，应关联行程组合匹配
        let mut inv = make_city_transport_invoice("inv1", 50.00);
        inv.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
            incomplete_fields: vec![],
        }];
        let toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        // 只有一笔 60 元支付（行程+高速费组合）
        let payment = make_payment_at("p1", 60.00, "2025-01-15 09:35");

        let result = batch_match(&[inv, toll], &[payment], 1.00);

        assert_eq!(result.matched.len(), 2);
        let toll_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::Toll).unwrap();
        // 高速费组合匹配，共享行程支付
        assert_eq!(toll_match.shared_from_invoice_id, Some("inv1".to_string()));
        assert_eq!(toll_match.payment_ids, vec!["p1".to_string()]);
        // 高速费永远一对一（即使共享也是 OneToOne）
        assert!(matches!(toll_match.match_type, MatchType::OneToOne));
    }

    #[test]
    fn test_toll_independent_match_does_not_block_trip() {
        // 高速费先单独匹配占用自己的支付，行程再匹配自己的支付
        let mut inv = make_city_transport_invoice("inv1", 50.00);
        inv.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
            incomplete_fields: vec![],
        }];
        let toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        let payments = vec![
            make_payment_at("p_toll", 10.00, "2025-01-15 09:35"),
            make_payment_at("p_trip", 50.00, "2025-01-15 09:10"),
        ];

        let result = batch_match(&[inv, toll], &payments, 1.00);

        let trip_match = result.matched.iter().find(|m| m.invoice.category == InvoiceCategory::CityTransport).unwrap();
        assert_eq!(trip_match.payment_ids, vec!["p_trip".to_string()]);
    }

    #[test]
    fn test_toll_no_match_goes_unmatched() {
        // 高速费金额与任何支付都不匹配，且无行程可关联
        let toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        let payment = make_payment_at("p1", 99.00, "2025-01-15 09:35");

        let result = batch_match(&[toll], &[payment], 1.00);

        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 1);
        assert_eq!(result.unmatched_invoices[0].id, "toll1");
    }

    #[test]
    fn test_trip_tolerates_payment_more_than_invoice() {
        // 行程 50 元，支付 60 元（多 10 元可能是未开票高速费），无高速费发票
        // 行程仍应匹配（toll_best 容差：差额 <= 行程金额且时间相近）
        let mut inv = make_city_transport_invoice("inv1", 50.00);
        inv.itineraries = vec![Itinerary {
            date_time: "2025-01-15 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 50.00,
            incomplete_fields: vec![],
        }];
        let payment = make_payment_at("p1", 60.00, "2025-01-15 09:10");

        let result = batch_match(&[inv], &[payment], 1.00);

        // 行程应匹配成功（容忍多出 10 元）
        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].payment_ids, vec!["p1".to_string()]);
    }

    #[test]
    fn test_toll_independent_match_uses_travel_time_not_invoice_date() {
        // 高速费通行时间 2025-01-15 09:30，开票日期 2025-01-20（ETC延迟5天）
        // 支付1：10元，2025-01-15 09:35（通行时间附近，应优先匹配）
        // 支付2：10元，2025-01-20 12:00（开票日期附近，不应匹配）
        let mut toll = make_toll_invoice("toll1", 10.00, "2025-01-15 09:30:00");
        toll.date = NaiveDate::from_ymd_opt(2025, 1, 20).unwrap();  // 开票日期延迟5天
        let payments = vec![
            make_payment_at("p_travel", 10.00, "2025-01-15 09:35"),
            make_payment_at("p_invoice", 10.00, "2025-01-20 12:00"),
        ];

        let result = batch_match(&[toll], &payments, 1.00);

        assert_eq!(result.matched.len(), 1);
        // 应匹配通行时间附近的支付，而非开票日期附近的
        assert_eq!(result.matched[0].payment_ids, vec!["p_travel".to_string()]);
    }

    #[test]
    fn test_insurance_filters_out_payment_after_invoice_date() {
        let invoice = make_invoice_at("ins1", 30.00, InvoiceCategory::Insurance, "2025-01-20");
        let payments = vec![
            make_payment_at("p_after", 30.00, "2025-01-21 12:00"),
        ];
        let result = batch_match(&[invoice], &payments, 1.00);
        assert_eq!(result.matched.len(), 0);
        assert_eq!(result.unmatched_invoices.len(), 1);
    }

    #[test]
    fn test_insurance_matches_payment_before_invoice_date() {
        let invoice = make_invoice_at("ins1", 30.00, InvoiceCategory::Insurance, "2025-01-20");
        let payments = vec![
            make_payment_at("p_real", 30.00, "2025-01-10 14:00"),
        ];
        let result = batch_match(&[invoice], &payments, 1.00);
        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].payment_ids, vec!["p_real".to_string()]);
    }

    #[test]
    fn test_insurance_prefers_before_over_after_when_both_match_amount() {
        let invoice = make_invoice_at("ins1", 30.00, InvoiceCategory::Insurance, "2025-01-20");
        let payments = vec![
            make_payment_at("p_before", 30.00, "2025-01-10 14:00"),
            make_payment_at("p_after", 30.00, "2025-01-21 12:00"),
        ];
        let result = batch_match(&[invoice], &payments, 1.00);
        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].payment_ids, vec!["p_before".to_string()]);
    }

    #[test]
    fn test_insurance_same_day_payment_matched() {
        let invoice = make_invoice_at("ins1", 30.00, InvoiceCategory::Insurance, "2025-01-20");
        let payments = vec![
            make_payment_at("p_same_day", 30.00, "2025-01-20 10:00"),
        ];
        let result = batch_match(&[invoice], &payments, 1.00);
        assert_eq!(result.matched.len(), 1);
        assert_eq!(result.matched[0].payment_ids, vec!["p_same_day".to_string()]);
    }
}
