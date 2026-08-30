use crate::models::payment::{PaymentRecord, PaymentSource};
use uuid::Uuid;

pub fn parse_alipay_bill(file_path: &str) -> Result<Vec<PaymentRecord>, String> {
    let bytes = std::fs::read(file_path).map_err(|e| format!("读取支付宝账单文件失败: {}", e))?;

    let (content, _encoding, _had_errors) = encoding_rs::GBK.decode(&bytes);
    let content = content.into_owned();

    // 第一遍解析：收集所有记录
    struct RawRecord {
        transaction_time: String,
        transaction_type: String,
        merchant_name: String,
        amount: f64,
        payment_method: String,
        transaction_id: String,
        merchant_order: String,
        is_refund: bool,
    }

    let mut raw_records: Vec<RawRecord> = Vec::new();
    let mut header_found = false;

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('-') {
            continue;
        }

        if !header_found {
            if line.contains("交易时间") && line.contains("金额") {
                header_found = true;
            }
            continue;
        }

        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 8 {
            continue;
        }

        let transaction_time = fields[0].trim().to_string();
        let transaction_type = fields[1].trim().to_string();
        let merchant_name = fields[2].trim().to_string();
        let direction = fields[5].trim();
        let amount_str = fields[6].trim();
        let payment_method = fields
            .get(7)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let status = fields.get(8).map(|s| s.trim()).unwrap_or_default();

        // 交易关闭的订单资金未成立（已原路退回或从未扣款成功），不能作为支付记录
        if status.contains("交易关闭") {
            continue;
        }

        let amount: f64 = amount_str
            .replace("¥", "")
            .replace("￥", "")
            .replace(",", "")
            .trim()
            .parse()
            .unwrap_or(0.0);

        if amount <= 0.0 {
            continue;
        }

        let is_refund = transaction_type.contains("退款");

        let signed_amount = if is_refund {
            -amount
        } else if direction.contains("收入") {
            -amount
        } else if direction.contains("支出") || direction.contains("支付") {
            amount
        } else {
            continue;
        };

        let transaction_id = if fields.len() > 9 {
            fields[9].trim().to_string()
        } else {
            Uuid::new_v4().to_string()
        };
        let merchant_order = if fields.len() > 10 {
            fields[10].trim().to_string()
        } else {
            String::new()
        };

        raw_records.push(RawRecord {
            transaction_time,
            transaction_type,
            merchant_name,
            amount: signed_amount,
            payment_method,
            transaction_id,
            merchant_order,
            is_refund,
        });
    }

    // 第二遍：退款关联。不做分隔符白名单——退款单号是“原交易单号 + 分隔符 + 后缀”
    // 的派生形式（实际账单中分隔符有 * 和 _ 两种），因此按前缀关系匹配；
    // 商家订单号精确相等作为兜底键，覆盖退款单号不再嵌入原单号的格式。
    // 两键都失败时退款丢弃（说明原支付不在本账单内，无需求可抵扣）。
    fn resolve_refund_target<'a>(
        refund: &RawRecord,
        originals: &[&'a RawRecord],
    ) -> Option<&'a RawRecord> {
        // 商家订单号精确相等；同名多笔时用交易单号前缀收窄
        let mut candidates: Vec<&RawRecord> = originals
            .iter()
            .copied()
            .filter(|o| !o.merchant_order.is_empty() && o.merchant_order == refund.merchant_order)
            .collect();
        if candidates.len() > 1 {
            candidates.retain(|o| refund.transaction_id.starts_with(&o.transaction_id));
        }
        // 兜底：交易单号前缀关系（兼容 * / _ / 无分隔符）
        if candidates.is_empty() {
            candidates.extend(
                originals
                    .iter()
                    .copied()
                    .filter(|o| refund.transaction_id.starts_with(&o.transaction_id)),
            );
        }
        // 同一交易单号在账单内理论上唯一，重复时取首条保证确定性
        candidates.into_iter().next()
    }

    let originals: Vec<&RawRecord> = raw_records.iter().filter(|r| !r.is_refund).collect();
    let mut refund_totals: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    for rec in raw_records.iter().filter(|r| r.is_refund) {
        if let Some(target) = resolve_refund_target(rec, &originals) {
            *refund_totals
                .entry(target.transaction_id.clone())
                .or_default() += rec.amount.abs();
        }
    }

    // 第三遍：输出最终记录，处理退款抵消
    let mut records = Vec::new();
    for rec in &raw_records {
        if rec.is_refund {
            // 退款记录本身不输出为独立记录
            continue;
        }

        // 检查是否有对应的退款
        let refund_total: f64 = refund_totals
            .get(&rec.transaction_id)
            .copied()
            .unwrap_or(0.0);

        if refund_total >= rec.amount.abs() {
            // 全额退款，跳过原始支付记录
            continue;
        }

        // 部分退款：扣除退款金额
        let final_amount = rec.amount - refund_total;
        if final_amount == 0.0 {
            continue;
        }

        records.push(PaymentRecord {
            id: Uuid::new_v4().to_string(),
            transaction_id: rec.transaction_id.clone(),
            transaction_time: rec.transaction_time.clone(),
            amount: final_amount,
            original_amount: rec.amount.abs(),
            refund_amount: refund_total,
            discount: 0.0,
            merchant_name: rec.merchant_name.clone(),
            source: PaymentSource::Alipay,
            category: rec.transaction_type.clone(),
            payment_method: rec.payment_method.clone(),
        });
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_file() {
        let result = parse_alipay_bill("nonexistent.csv");
        assert!(result.is_err());
    }

    /// 解析器按 GBK 解码账单文件，测试样本需以 GBK 编码落盘
    fn write_gbk_bill(name: &str, content: &str) -> String {
        let (bytes, _, _) = encoding_rs::GBK.encode(content);
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, &bytes).unwrap();
        path.to_string_lossy().into_owned()
    }

    const REFUND_FORMAT_CSV: &str = "\
交易时间,交易分类,交易对方,对方账号,商品说明,收/支,金额,收/付款方式,交易状态,交易订单号,商家订单号,备注,
2026-08-03 12:40:09,酒店旅游,汉庭酒店,/,星程酒店住宿,支出,3935.65,花呗,交易成功,2026080322001400311452585041,71d8ee908d8145a7,
2026-08-10 13:06:55,退款,汉庭酒店,/,退款-星程酒店住宿,不计收支,1412.60,花呗,退款成功,2026080322001400311452585041_ea4a93c75ab4485,71d8ee908d8145a7,
2026-08-24 00:00:00,餐饮美食,淘宝闪购,/,外卖订单,支出,14.10,花呗,交易成功,2026082423001100311442871273,1300060132608240,
2026-08-24 00:37:33,退款,淘宝闪购,/,退款-外卖订单,不计收支,14.10,花呗,退款成功,2026082423001100311442871273*1300060132608240,other-merchant-order,
2026-08-01 12:00:00,餐饮美食,面馆,/,牛肉面,支出,30.00,余额,交易成功,20260801NOODLE,mo-noodle,
2026-08-01 13:00:00,退款,面馆,/,退款-牛肉面,不计收支,30.00,余额,退款成功,UNRELATED-REFUND-ID,mo-noodle,
2026-08-04 20:00:00,演出赛事,大麦,/,演出票,支出,2236.00,花呗,交易成功,2026080423001400311402228803,11190600726080478,
2026-08-05 10:45:23,退款,大麦,/,退款-演出票,不计收支,1118.00,花呗,退款成功,2026080423001400311402228803_aaa,11190600726080478,
2026-08-05 10:45:24,退款,大麦,/,退款-演出票,不计收支,1118.00,花呗,退款成功,2026080423001400311402228803_bbb,11190600726080478,
2026-08-28 19:22:48,酒店旅游,网鱼酒店,/,房费,支出,300.00,花呗,交易关闭,2026082823001400311412842016,70126600004890,
";

    #[test]
    fn test_refund_association_formats_and_closed_orders() {
        // 覆盖真实账单中出现的全部关联形态：
        // 1. 下划线分隔的退款单号（部分退款，按净额保留）
        // 2. 星号分隔的退款单号（商家订单号不一致，靠前缀关联，全额退款后剔除）
        // 3. 退款单号与原单无前缀关系，靠商家订单号兜底关联（全额退款后剔除）
        // 4. 同一原支付的多笔退款累计抵扣（全额退款后剔除）
        // 5. 交易关闭的废单直接剔除，不进入支付池
        let path = write_gbk_bill("alipay_refund_assoc_test.csv", REFUND_FORMAT_CSV);
        let records = parse_alipay_bill(&path).unwrap();

        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(r.merchant_name, "汉庭酒店");
        assert!((r.amount - 2523.05).abs() < 1e-9);
        assert!((r.original_amount - 3935.65).abs() < 1e-9);
        assert!((r.refund_amount - 1412.60).abs() < 1e-9);
    }

    /// 真实账单联调：设置 ALIPAY_REAL_BILL 环境变量后运行
    /// `ALIPAY_REAL_BILL=<csv路径> cargo test --lib debug_real_bill -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn debug_real_bill_refund_assoc() {
        let Ok(path) = std::env::var("ALIPAY_REAL_BILL") else {
            println!("未设置 ALIPAY_REAL_BILL，跳过真实账单联调");
            return;
        };
        let records = parse_alipay_bill(&path).unwrap();
        println!("parsed records: {}", records.len());
        let assoc = records.iter().filter(|r| r.refund_amount > 0.0).count();
        println!("records with associated refund: {}", assoc);
        for r in &records {
            if r.refund_amount > 0.0 {
                println!(
                    "{} | net={} orig={} refund={} | {}",
                    r.transaction_time, r.amount, r.original_amount, r.refund_amount, r.merchant_name
                );
            }
        }
    }
}
