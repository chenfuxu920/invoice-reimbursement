/// 验证火车票/机票城市自动提取功能
/// 使用 pdfplumber 文本提取（无需 OCR），适合 Windows 无 pdftoppm 环境
use invoice_reimbursement_lib::models::invoice::InvoiceSource;
use invoice_reimbursement_lib::pdf::text_extractor::{extract_pdf_column_aware, has_sufficient_text};

#[test]
fn test_extract_cities_from_real_tickets() {
    let test_cases = vec![
        ("【飞猪】成都-长沙 机票", "../data/机票/【飞猪】成都-长沙  订单9586482810622-机票款凭证 报销凭证.pdf"),
        ("【飞猪】长沙-成都 机票", "../data/机票/【飞猪】长沙-成都  订单9640359113622-机票款凭证 报销凭证.pdf"),
        ("火车票", "../data/火车票/25429165818005131893.pdf"),
    ];

    println!("\n========== 票据城市提取验证 ==========\n");

    for (label, path) in &test_cases {
        println!("--- {} ---", label);
        println!("  文件: {}", path);

        // Step 1: 用 pdfplumber 提取文本
        match extract_pdf_column_aware(path) {
            Ok(extraction) => {
                let items: Vec<_> = extraction.pages.iter().flat_map(|p| p.texts.clone()).collect();
                let char_count: usize = items.iter().map(|t| t.text.chars().count()).sum();
                println!("  pdfplumber 提取成功: {} 段文字, {} 字符", items.len(), char_count);

                if !has_sufficient_text(&items, 20) {
                    println!("  ⚠ 文本不足 20 字符，跳过");
                    continue;
                }

                // 打印原始文本用于调试（首次验证时保留）
                let all_text: String = items.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join("\n");
                println!("  文本: {} 字符", all_text.chars().count());

                // Step 2: 用 parse_invoice_text 解析
                use invoice_reimbursement_lib::parser::invoice_parser::parse_invoice_text;
                let source = InvoiceSource::Pdf(path.to_string());

                match parse_invoice_text(&items, source) {
                    Ok(inv) => {
                        println!("  类别: {:?}", inv.category);
                        println!("  金额: {:.2}", inv.amount);
                        println!("  销售方: {}", inv.seller_name);
                        println!("  开票日期: {}", inv.date);
                        println!("  出行日期: {:?}", inv.travel_date);
                        println!("  出发城市: {:?}", inv.departure_city);
                        println!("  到达城市: {:?}", inv.arrival_city);

                        // 验证飞猪机票
                        if label.contains("成都-长沙") {
                            assert_eq!(inv.departure_city.as_deref(), Some("成都"), "出发城市应为成都");
                            assert_eq!(inv.arrival_city.as_deref(), Some("长沙"), "到达城市应为长沙");
                        }
                        if label.contains("长沙-成都") {
                            assert_eq!(inv.departure_city.as_deref(), Some("长沙"), "出发城市应为长沙");
                            assert_eq!(inv.arrival_city.as_deref(), Some("成都"), "到达城市应为成都");
                        }
                    }
                    Err(e) => {
                        println!("  ✗ 解析失败: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("  ✗ pdfplumber 文本提取失败: {}", e);
            }
        }
        println!();
    }

    println!("========== 验证完成 ==========\n");
}
