use invoice_reimbursement_lib::parser::{parse_bill_auto, alipay_parser, wechat_parser};

const BILL_DIR: &str = r"C:\Projects\rust-projects\invoice-reimbursement\data\账单";

#[test]
fn scratch_verify_bill_detection() {
    let entries = std::fs::read_dir(BILL_DIR).unwrap();
    for e in entries.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        println!("\n===== {} =====", name);
        let ext = p.extension().unwrap_or_default().to_string_lossy().to_lowercase();

        // auto detect
        match parse_bill_auto(p.to_str().unwrap()) {
            Ok(recs) => {
                let wechat = recs.iter().filter(|r| matches!(r.source, invoice_reimbursement_lib::models::payment::PaymentSource::Wechat)).count();
                let alipay = recs.iter().filter(|r| matches!(r.source, invoice_reimbursement_lib::models::payment::PaymentSource::Alipay)).count();
                println!("auto: total={} wechat={} alipay={}", recs.len(), wechat, alipay);
                for r in recs.iter().take(5) {
                    println!("  {:?} | {} | {} | {}", r.source, r.merchant_name, r.amount, r.transaction_time);
                }
            }
            Err(e) => println!("auto: ERR {}", e),
        }

        // wechat parser on same file (hypothesis: parses alipay csv)
        match wechat_parser::parse_wechat_bill(p.to_str().unwrap()) {
            Ok(recs) => println!("wechat_parser: parsed {} records", recs.len()),
            Err(e) => println!("wechat_parser: ERR {}", e),
        }

        // alipay parser on same file
        match alipay_parser::parse_alipay_bill(p.to_str().unwrap()) {
            Ok(recs) => println!("alipay_parser: parsed {} records", recs.len()),
            Err(e) => println!("alipay_parser: ERR {}", e),
        }

        let _ = ext;
    }
}
