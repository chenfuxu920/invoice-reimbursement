use invoice_reimbursement_lib::matching::batch::batch_match;
use invoice_reimbursement_lib::ocr::OcrEngine;
use invoice_reimbursement_lib::parser::alipay_parser;
use invoice_reimbursement_lib::parser::wechat_parser;
use invoice_reimbursement_lib::pdf::form_builder::build_reimbursement_form;
use invoice_reimbursement_lib::pdf::form_html_generator::generate_reimbursement_html_string;
use invoice_reimbursement_lib::pdf::comparison_image_pdf_generator;
use invoice_reimbursement_lib::pdf::invoice_pipeline::{parse_all_from_dir, ExtractionConfig};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 4 {
        eprintln!("用法: generate_report <发票目录> <账单目录> <输出目录>");
        eprintln!("示例: generate_report ../data/发票与行程单 ../data/账单 ../data");
        std::process::exit(1);
    }

    let invoice_dir = &args[1];
    let bill_dir = &args[2];
    let output_dir = &args[3];

    // 检查目录存在
    if !Path::new(invoice_dir).exists() {
        eprintln!("发票目录不存在: {}", invoice_dir);
        std::process::exit(1);
    }
    if !Path::new(bill_dir).exists() {
        eprintln!("账单目录不存在: {}", bill_dir);
        std::process::exit(1);
    }
    std::fs::create_dir_all(output_dir).expect("无法创建输出目录");

    // 1. 初始化 OCR 引擎
    println!("正在初始化 OCR 引擎...");
    let mut engine = OcrEngine::new("models").expect("OCR 初始化失败，请确保 models 目录存在");

    // 2. 解析所有文件（发票+行程单）
    println!("\n=== 解析文件 ({}) ===", invoice_dir);
    let result = parse_all_from_dir(invoice_dir, &mut engine, &ExtractionConfig::default());
    let invoices = result.invoices;
    println!("  成功: {}, 失败: {}", invoices.len(), result.errors.len());
    for inv in &invoices {
        let has_itinerary = if inv.itineraries.is_empty() { "" } else { " [已关联行程单]" };
        println!("  ✓ {} 类别={:?} 金额={:.2}{}", inv.invoice_number, inv.category, inv.amount, has_itinerary);
    }
    for (name, err) in &result.errors {
        println!("  ✗ {} - {}", name, err);
    }

    if invoices.is_empty() {
        eprintln!("未解析到任何发票，退出");
        std::process::exit(1);
    }

    // 3. 导入账单
    println!("\n=== 导入账单 ({}) ===", bill_dir);
    let mut payments = Vec::new();

    // 自动查找 xlsx 和 csv 文件
    if let Ok(entries) = std::fs::read_dir(bill_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let ext = path.extension().unwrap_or_default().to_str().unwrap_or_default();
            match ext {
                "xlsx" => {
                    println!("  导入: {}", path.file_name().unwrap().to_str().unwrap());
                    match wechat_parser::parse_wechat_bill(path.to_str().unwrap()) {
                        Ok(records) => {
                            println!("    → {} 条记录", records.len());
                            payments.extend(records);
                        }
                        Err(e) => println!("    ✗ 失败: {}", e),
                    }
                }
                "csv" => {
                    println!("  导入: {}", path.file_name().unwrap().to_str().unwrap());
                    match alipay_parser::parse_alipay_bill(path.to_str().unwrap()) {
                        Ok(records) => {
                            println!("    → {} 条记录", records.len());
                            payments.extend(records);
                        }
                        Err(e) => println!("    ✗ 失败: {}", e),
                    }
                }
                _ => {}
            }
        }
    }
    println!("  账单总计: {} 条", payments.len());

    // 4. 匹配
    println!("\n=== 自动匹配 ===");
    let match_result = batch_match(&invoices, &payments, 5.0);
    println!("  已匹配: {} 组", match_result.matched.len());
    println!("  未匹配发票: {} 张", match_result.unmatched_invoices.len());
    println!("  未匹配支付: {} 条", match_result.unmatched_payments.len());

    for m in &match_result.matched {
        println!("  ✓ {} ({}) → {:.2}", m.invoice.seller_name, m.invoice.invoice_number, m.invoice.amount);
    }
    for inv in &match_result.unmatched_invoices {
        println!("  ✗ 未匹配发票: {} - {:.2} - {:?}", inv.invoice_number, inv.amount, inv.category);
    }

    // 5. 构建报销单
    println!("\n=== 生成报销单 ===");
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

    println!("  姓名: {}", form.name);
    println!("  出差天数: {}", form.travel_days);
    println!("  城市间交通费: {:.2}", form.transport_subtotal);
    println!("  市内交通费: {:.2} ({}张)", form.city_transport_amount, form.city_transport_count);
    for h in &form.hotel_levels {
        println!("  住宿费: {}晚 标准{:.2}/晚 可报销{:.2} 实际{:.2}",
                 h.days, h.daily_rate, h.amount, h.actual_amount);
    }
    println!("  伙食补助: {:.2} ({}天×{:.2})", form.meal_subsidy.amount, form.meal_subsidy.days, form.meal_subsidy.daily_rate);
    println!("  总额: {:.2}", form.total_amount);

    // 6. 生成输出文件
    let html = generate_reimbursement_html_string(&form);
    let html_path = format!("{}/报销单.html", output_dir);
    std::fs::write(&html_path, &html).expect("写入 HTML 失败");
    println!("  报销单 HTML: {}", html_path);

    // 生成对照单 PDF（含发票图片）
    let image_pdf_path = format!("{}/对照表_含图片.pdf", output_dir);
    match comparison_image_pdf_generator::generate_comparison_image_pdf(
        &match_result.matched,
        invoice_dir,
        &image_pdf_path,
        400,
        Some(&form.destination),
    ) {
        Ok(_) => println!("  对照单 PDF(含图片): {}", image_pdf_path),
        Err(e) => println!("  对照单 PDF(含图片) 生成失败: {}", e),
    }

    println!("\n完成 ✓");
}
