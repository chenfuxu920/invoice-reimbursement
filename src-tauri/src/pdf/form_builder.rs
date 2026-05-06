use crate::models::match_result::MatchResult;
use crate::models::invoice::InvoiceCategory;
use crate::models::reimbursement::{ReimbursementForm, CategorySummary};
use std::collections::HashMap;

/// 从匹配结果构建报销表单
pub fn build_reimbursement_form(
    match_results: &[MatchResult],
    name: &str,
    department: &str,
    travel_start: &str,
    travel_end: &str,
    companions: usize,
) -> ReimbursementForm {
    let mut category_map: HashMap<InvoiceCategory, (usize, f64)> = HashMap::new();

    for result in match_results {
        let cat = &result.invoice.category;
        let entry = category_map.entry(cat.clone()).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += result.invoice.amount;
    }

    let mut summaries: Vec<CategorySummary> = category_map
        .into_iter()
        .map(|(category, (count, total_amount))| CategorySummary {
            category,
            count,
            total_amount,
        })
        .collect();

    // 按固定顺序排列
    let order = [
        InvoiceCategory::Train,
        InvoiceCategory::Flight,
        InvoiceCategory::TicketChange,
        InvoiceCategory::CityTransport,
        InvoiceCategory::Hotel,
        InvoiceCategory::Meal,
        InvoiceCategory::Other,
    ];

    summaries.sort_by_key(|s| {
        order.iter().position(|c| c == &s.category).unwrap_or(99)
    });

    let total_amount: f64 = summaries.iter().map(|s| s.total_amount).sum();

    ReimbursementForm {
        name: name.to_string(),
        department: department.to_string(),
        travel_start: travel_start.to_string(),
        travel_end: travel_end.to_string(),
        companions,
        summaries,
        total_amount,
    }
}
