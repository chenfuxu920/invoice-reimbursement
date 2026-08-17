use invoice_reimbursement_lib::models::payment::PaymentSource;
use invoice_reimbursement_lib::parser;

#[test]
fn debug_bill_auto_detect() {
    let alipay = r"C:\Projects\rust-projects\invoice-reimbursement\data\账单\支付宝交易明细(20260417-20260517).csv";
    let wechat = r"C:\Projects\rust-projects\invoice-reimbursement\data\账单\微信支付账单流水文件(20260418-20260518)_20260518094032.xlsx";

    match parser::parse_bill_auto(alipay) {
        Ok(records) => {
            eprintln!("ALIPAY OK: {} records", records.len());
            for r in records.iter().take(3) {
                eprintln!(
                    "  source={:?} merchant={} amount={}",
                    r.source, r.merchant_name, r.amount
                );
            }
            assert!(!records.is_empty(), "支付宝账单解析出 0 条记录");
            assert!(
                records
                    .iter()
                    .all(|r| matches!(r.source, PaymentSource::Alipay)),
                "支付宝账单来源必须全部为 Alipay"
            );
        }
        Err(e) => eprintln!("ALIPAY ERR: {}", e),
    }

    match parser::parse_bill_auto(wechat) {
        Ok(records) => {
            eprintln!("WECHAT OK: {} records", records.len());
            for r in records.iter().take(3) {
                eprintln!(
                    "  source={:?} merchant={} amount={}",
                    r.source, r.merchant_name, r.amount
                );
            }
            assert!(!records.is_empty(), "微信账单解析出 0 条记录");
            assert!(
                records
                    .iter()
                    .all(|r| matches!(r.source, PaymentSource::Wechat)),
                "微信账单来源必须全部为 Wechat"
            );
        }
        Err(e) => eprintln!("WECHAT ERR: {}", e),
    }
}
