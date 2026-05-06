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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{Invoice, InvoiceSource};
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
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            category,
            source: InvoiceSource::Pdf("test.pdf".to_string()),
            itineraries: vec![],
        }
    }

    fn make_payment(id: &str, amount: f64) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            transaction_id: format!("TX-{}", id),
            transaction_time: "2025-01-01 12:00".to_string(),
            amount,
            merchant_name: "Test Merchant".to_string(),
            source: PaymentSource::Wechat,
            category: "交通".to_string(),
        }
    }

    fn make_match_result(invoice_id: &str, invoice_amount: f64, category: InvoiceCategory) -> MatchResult {
        let invoice = make_invoice(invoice_id, invoice_amount, category);
        let payment = make_payment(&format!("pay-{}", invoice_id), invoice_amount);
        MatchResult {
            invoice_id: invoice.id.clone(),
            invoice,
            payment_ids: vec![payment.id.clone()],
            payments: vec![payment],
            match_type: MatchType::OneToOne,
            confidence: 1.0,
            amount_diff: 0.0,
        }
    }

    #[test]
    fn test_build_form_empty_results() {
        let form = build_reimbursement_form(
            &[],
            "张三",
            "技术部",
            "2025-01-01",
            "2025-01-05",
            0,
        );
        assert_eq!(form.name, "张三");
        assert_eq!(form.department, "技术部");
        assert_eq!(form.travel_start, "2025-01-01");
        assert_eq!(form.travel_end, "2025-01-05");
        assert_eq!(form.companions, 0);
        assert!(form.summaries.is_empty());
        assert!((form.total_amount - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_build_form_single_category() {
        let results = vec![
            make_match_result("inv1", 100.0, InvoiceCategory::Train),
            make_match_result("inv2", 200.0, InvoiceCategory::Train),
        ];
        let form = build_reimbursement_form(
            &results,
            "李四",
            "市场部",
            "2025-02-01",
            "2025-02-03",
            1,
        );
        assert_eq!(form.summaries.len(), 1);
        assert_eq!(form.summaries[0].category, InvoiceCategory::Train);
        assert_eq!(form.summaries[0].count, 2);
        assert!((form.summaries[0].total_amount - 300.0).abs() < 0.01);
        assert!((form.total_amount - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_build_form_multiple_categories() {
        let results = vec![
            make_match_result("inv1", 500.0, InvoiceCategory::Flight),
            make_match_result("inv2", 300.0, InvoiceCategory::Hotel),
            make_match_result("inv3", 80.0, InvoiceCategory::CityTransport),
            make_match_result("inv4", 150.0, InvoiceCategory::Meal),
        ];
        let form = build_reimbursement_form(
            &results,
            "王五",
            "财务部",
            "2025-03-01",
            "2025-03-05",
            2,
        );
        assert_eq!(form.summaries.len(), 4);
        // 验证排序：Flight(1) -> CityTransport(3) -> Hotel(4) -> Meal(5)
        assert_eq!(form.summaries[0].category, InvoiceCategory::Flight);
        assert_eq!(form.summaries[1].category, InvoiceCategory::CityTransport);
        assert_eq!(form.summaries[2].category, InvoiceCategory::Hotel);
        assert_eq!(form.summaries[3].category, InvoiceCategory::Meal);
        // 验证总金额
        assert!((form.total_amount - 1030.0).abs() < 0.01);
    }

    #[test]
    fn test_build_form_category_summary_amounts() {
        let results = vec![
            make_match_result("inv1", 50.5, InvoiceCategory::Meal),
            make_match_result("inv2", 30.0, InvoiceCategory::Meal),
            make_match_result("inv3", 200.0, InvoiceCategory::Hotel),
        ];
        let form = build_reimbursement_form(
            &results,
            "赵六",
            "人事部",
            "2025-04-01",
            "2025-04-02",
            0,
        );
        // 验证 Meal 类别汇总
        let meal_summary = form.summaries.iter().find(|s| s.category == InvoiceCategory::Meal).unwrap();
        assert_eq!(meal_summary.count, 2);
        assert!((meal_summary.total_amount - 80.5).abs() < 0.01);

        // 验证 Hotel 类别汇总
        let hotel_summary = form.summaries.iter().find(|s| s.category == InvoiceCategory::Hotel).unwrap();
        assert_eq!(hotel_summary.count, 1);
        assert!((hotel_summary.total_amount - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_build_form_category_ordering() {
        // 验证类别按固定顺序排列：Train, Flight, TicketChange, CityTransport, Hotel, Meal, Other
        let results = vec![
            make_match_result("inv1", 10.0, InvoiceCategory::Other),
            make_match_result("inv2", 20.0, InvoiceCategory::Meal),
            make_match_result("inv3", 30.0, InvoiceCategory::Hotel),
            make_match_result("inv4", 40.0, InvoiceCategory::CityTransport),
            make_match_result("inv5", 50.0, InvoiceCategory::TicketChange),
            make_match_result("inv6", 60.0, InvoiceCategory::Flight),
            make_match_result("inv7", 70.0, InvoiceCategory::Train),
        ];
        let form = build_reimbursement_form(
            &results,
            "测试",
            "测试部",
            "2025-01-01",
            "2025-01-10",
            0,
        );
        assert_eq!(form.summaries.len(), 7);
        assert_eq!(form.summaries[0].category, InvoiceCategory::Train);
        assert_eq!(form.summaries[1].category, InvoiceCategory::Flight);
        assert_eq!(form.summaries[2].category, InvoiceCategory::TicketChange);
        assert_eq!(form.summaries[3].category, InvoiceCategory::CityTransport);
        assert_eq!(form.summaries[4].category, InvoiceCategory::Hotel);
        assert_eq!(form.summaries[5].category, InvoiceCategory::Meal);
        assert_eq!(form.summaries[6].category, InvoiceCategory::Other);
    }

    #[test]
    fn test_build_form_total_amount() {
        let results = vec![
            make_match_result("inv1", 553.0, InvoiceCategory::Train),
            make_match_result("inv2", 1200.0, InvoiceCategory::Flight),
            make_match_result("inv3", 450.0, InvoiceCategory::Hotel),
        ];
        let form = build_reimbursement_form(
            &results,
            "钱七",
            "运营部",
            "2025-05-01",
            "2025-05-10",
            3,
        );
        assert!((form.total_amount - 2203.0).abs() < 0.01);
        assert_eq!(form.companions, 3);
    }
}
