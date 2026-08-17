/// 端到端测试：读取 data 目录所有真实发票 → 解析 → 匹配 → 生成报销单
use invoice_reimbursement_lib::matching::batch::batch_match;
use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::parser::alipay_parser;
use invoice_reimbursement_lib::parser::wechat_parser;
use invoice_reimbursement_lib::pdf::comparison_generator::generate_comparison_pdf;
use invoice_reimbursement_lib::pdf::form_builder::build_reimbursement_form;
use invoice_reimbursement_lib::pdf::form_generator::generate_reimbursement_pdf;
use invoice_reimbursement_lib::pdf::form_html_generator::generate_reimbursement_html_string;
use invoice_reimbursement_lib::pdf::invoice_pipeline::{parse_all_from_dir, ExtractionConfig};
use std::path::Path;

const MODELS_DIR: &str = "models";
const INVOICE_DIRS: &[&str] = &[
    "../data/市内交通",
    "../data/行程单/滴滴",
    "../data/行程单/天府通",
    "../data/行程单/高德",
    "../data/机票",
    "../data/退改签",
    "../data/住宿",
    "../data/保险",
    "../data/通行费",
    "../data/其他发票",
    "../data/未分类",
];
const BILL_DIR: &str = "../data/账单";

#[test]
#[ignore]
fn e2e_full_pipeline_from_real_files() {
    // 1. 初始化 OCR 引擎
    let mut engine = OcrEngine::new(MODELS_DIR).expect("OCR init failed");

    // 2. 解析所有文件（发票+行程单），行程单自动配对到对应发票
    let mut invoices = Vec::new();
    let mut all_errors = Vec::new();
    for invoice_dir in INVOICE_DIRS {
        let result = parse_all_from_dir(invoice_dir, &mut engine, &ExtractionConfig::default());
        invoices.extend(result.invoices);
        all_errors.extend(result.errors);
    }
    println!("\n=== 解析结果 ===");
    println!("  成功: {}, 失败: {}", invoices.len(), all_errors.len());
    for inv in &invoices {
        let has_itinerary = if inv.itineraries.is_empty() {
            ""
        } else {
            " [已关联行程单]"
        };
        println!(
            "  ✓ {} 类别={:?} 金额={:.2}{}",
            inv.invoice_number, inv.category, inv.amount, has_itinerary
        );
        if !inv.itineraries.is_empty() {
            for it in &inv.itineraries {
                println!(
                    "     行程: {} {} {:.2}元",
                    it.date_time, it.provider, it.amount
                );
            }
        }
    }
    for (name, err) in &all_errors {
        println!("  ✗ {} - {}", name, err);
    }

    // 3. 导入账单（按前缀匹配，账单文件按月滚动更新）
    let mut payments = Vec::new();
    if let Some(p) = find_bill_file(BILL_DIR, "微信支付账单流水文件", "xlsx") {
        if let Ok(records) = wechat_parser::parse_wechat_bill(p.to_str().unwrap()) {
            payments.extend(records);
        } else {
            eprintln!("  ✗ 微信账单解析失败: {}", p.display());
        }
    } else {
        eprintln!("  ✗ 未找到微信账单文件 (*.xlsx)");
    }
    if let Some(p) = find_bill_file(BILL_DIR, "支付宝交易明细", "csv") {
        if let Ok(records) = alipay_parser::parse_alipay_bill(p.to_str().unwrap()) {
            payments.extend(records);
        } else {
            eprintln!("  ✗ 支付宝账单解析失败: {}", p.display());
        }
    } else {
        eprintln!("  ✗ 未找到支付宝账单文件 (*.csv)");
    }
    println!("\n=== 账单: {} 条 ===", payments.len());

    // 4. 匹配（带行程的 CityTransport 发票会按行程逐条匹配支付）
    let match_result = batch_match(&invoices, &payments, 5.0);
    println!(
        "\n=== 匹配: {} 组, 未匹配发票 {}, 未匹配支付 {} ===",
        match_result.matched.len(),
        match_result.unmatched_invoices.len(),
        match_result.unmatched_payments.len()
    );

    // 5. 构建报销单
    let form = build_reimbursement_form(
        &match_result.matched,
        "陈福旭",
        "聘用工程师",
        "四川省成都市",
        "2025-08-04",
        "2025-08-15",
        0,
        "其他人员",
    );

    // 6. 生成 HTML / PDF
    let html = generate_reimbursement_html_string(&form);
    let html_path = "../data/报销单_E2E真实数据.html";
    std::fs::write(html_path, &html).expect("Failed to write HTML");

    let pdf_path = "../data/报销单_E2E真实数据.pdf";
    let _ = generate_reimbursement_pdf(&form, pdf_path);

    let comparison_path = "../data/对照表_E2E真实数据.pdf";
    let _ = generate_comparison_pdf(
        &match_result.matched,
        &match_result
            .unmatched_invoices
            .iter()
            .map(|i| i.id.clone())
            .collect::<Vec<_>>(),
        &match_result
            .unmatched_payments
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>(),
        comparison_path,
    );

    // 7. 验证结构完整性（计算逻辑由 form_builder 单元测试覆盖）
    assert!(!invoices.is_empty(), "至少应解析出一张发票");
    assert!(
        match_result.matched.len() >= 8,
        "至少应匹配 8 组，实际 {}",
        match_result.matched.len()
    );
    assert!(form.total_amount > 0.0, "总额应大于 0");
    assert!(html.contains("差 旅 费 报 销 单"));

    assert!(!form.transport_details.is_empty(), "城市间交通费不应为空");
    assert!(form.transport_subtotal > 0.0);
    assert!(form.city_transport_count >= 1);
    assert!(form.city_transport_amount > 0.0);

    assert!(!form.hotel_levels.is_empty(), "住宿费不应为空");
    for h in &form.hotel_levels {
        assert!(h.days >= 1);
        assert!(h.amount > 0.0);
        assert!(h.amount <= h.actual_amount + 0.01);
    }

    assert!(form.meal_subsidy.days >= 1);
    assert!(form.meal_subsidy.amount > 0.0);

    assert!(html.contains(&form.name));
    assert!(html.contains(&format!("{:.2}", form.total_amount)));

    println!("\n  所有断言通过 ✓");
}

/// 在目录下按名称前缀找账单文件（账单按月更新，文件名日期段会变）
fn find_bill_file(dir: &str, prefix: &str, ext: &str) -> Option<std::path::PathBuf> {
    let dir_path = Path::new(dir);
    if !dir_path.is_dir() {
        return None;
    }
    std::fs::read_dir(dir_path)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.extension().and_then(|x| x.to_str()) == Some(ext)
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(prefix))
                    .unwrap_or(false)
        })
}
