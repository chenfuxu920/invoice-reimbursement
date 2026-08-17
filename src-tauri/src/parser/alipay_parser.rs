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

        raw_records.push(RawRecord {
            transaction_time,
            transaction_type,
            merchant_name,
            amount: signed_amount,
            payment_method,
            transaction_id,
            is_refund,
        });
    }

    // 第二遍：处理退款关联，按交易单号前缀匹配
    let mut refund_map: std::collections::HashMap<String, Vec<f64>> =
        std::collections::HashMap::new();
    for rec in &raw_records {
        if rec.is_refund {
            // 退款记录的交易单号格式：原始单号*后缀
            if let Some(pos) = rec.transaction_id.find('*') {
                let prefix = &rec.transaction_id[..pos];
                refund_map
                    .entry(prefix.to_string())
                    .or_default()
                    .push(rec.amount.abs());
            }
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
        let refund_total: f64 = refund_map
            .get(&rec.transaction_id)
            .map(|v| v.iter().sum())
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
}
