use crate::models::payment::{PaymentRecord, PaymentSource};
use calamine::{open_workbook_auto, Data, Reader};
use uuid::Uuid;

/// 将 Excel 日期序列号转为日期时间字符串 "YYYY-MM-DD HH:MM:SS"
fn excel_serial_to_datetime(serial: f64) -> String {
    let epoch = chrono::NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
    let days = serial.floor() as i64;
    let time_frac = serial - serial.floor();
    let date = epoch + chrono::Duration::days(days);
    let total_secs = (time_frac * 86400.0).round() as i64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!(
        "{} {:02}:{:02}:{:02}",
        date.format("%Y-%m-%d"),
        hours,
        minutes,
        seconds
    )
}

/// 读取单元格的值，将 DateTime 类型转为可读字符串
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::DateTime(dt) => excel_serial_to_datetime(dt.as_f64()),
        Data::DateTimeIso(s) => s.clone(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if *f > 40000.0 && *f < 55000.0 {
                excel_serial_to_datetime(*f)
            } else {
                f.to_string()
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

pub fn parse_wechat_bill(file_path: &str) -> Result<Vec<PaymentRecord>, String> {
    let mut workbook =
        open_workbook_auto(file_path).map_err(|e| format!("打开微信账单失败: {}", e))?;

    let sheet = workbook.sheet_names().get(0).ok_or("无工作表")?.clone();

    let range = workbook
        .worksheet_range(&sheet)
        .map_err(|e| format!("读取工作表失败: {}", e))?;

    let mut records = Vec::new();
    let mut header_found = false;

    for row in range.rows() {
        let first_cell = row.get(0).map(|c| cell_to_string(c)).unwrap_or_default();

        // 微信账单前几行是标题，找到 "交易时间" 那行开始读取
        if first_cell.contains("交易时间") {
            header_found = true;
            continue;
        }

        if !header_found {
            continue;
        }
        if row.len() < 9 {
            continue;
        }

        let transaction_time = row.get(0).map(|c| cell_to_string(c)).unwrap_or_default();
        let transaction_type = row.get(1).map(|c| cell_to_string(c)).unwrap_or_default();
        let merchant_name = row.get(2).map(|c| cell_to_string(c)).unwrap_or_default();
        let category = row.get(3).map(|c| cell_to_string(c)).unwrap_or_default();
        let direction = row.get(4).map(|c| cell_to_string(c)).unwrap_or_default();
        let amount_str = row.get(5).map(|c| cell_to_string(c)).unwrap_or_default();
        let payment_method = row.get(6).map(|c| cell_to_string(c)).unwrap_or_default();
        let status = row.get(7).map(|c| cell_to_string(c)).unwrap_or_default();
        let transaction_id = row.get(8).map(|c| cell_to_string(c)).unwrap_or_default();
        let remark = row.get(10).map(|c| cell_to_string(c)).unwrap_or_default();

        // 解析金额
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

        // 从状态列中提取退款金额（如 "已退款(¥2071.80)" 或 "已退¥2071.80"）
        let refund_amount = extract_refund_amount(&status);

        // 从备注列提取优惠金额（如 "已优惠¥0.80"）
        let discount_amount = extract_discount_amount(&remark);

        // 判断逻辑：
        // 1. "收入" + 交易类型含"退款" → 退款记录，金额为负
        // 2. "支出" + "已全额退款" → 跳过
        // 3. "支出" + 状态含退款金额 → 部分退款，计算净额 = 原支付 - 退款
        // 4. "支出" + "支付成功" → 正常支出
        let signed_amount = if direction.contains("收") && transaction_type.contains("退款") {
            -amount
        } else if direction.contains("支") && status.contains("全额退款") {
            continue;
        } else if direction.contains("支") && refund_amount > 0.0 {
            amount - refund_amount
        } else if direction.contains("支") {
            amount
        } else {
            continue;
        };

        // 跳过净额为0的记录
        if signed_amount.abs() < 0.01 {
            continue;
        }

        records.push(PaymentRecord {
            id: Uuid::new_v4().to_string(),
            transaction_id,
            transaction_time,
            amount: signed_amount,
            original_amount: amount,
            refund_amount,
            discount: discount_amount,
            merchant_name,
            source: PaymentSource::Wechat,
            category,
            payment_method,
        });
    }

    Ok(records)
}

/// 调试用：打印账单中包含指定关键词的行
pub fn debug_wechat_bill_filter(file_path: &str, keyword: &str) -> Result<(), String> {
    let mut workbook =
        open_workbook_auto(file_path).map_err(|e| format!("打开微信账单失败: {}", e))?;
    let sheet = workbook.sheet_names().get(0).ok_or("无工作表")?.clone();
    let range = workbook
        .worksheet_range(&sheet)
        .map_err(|e| format!("读取工作表失败: {}", e))?;

    for row in range.rows() {
        let first_cell = row.get(0).map(|c| c.to_string()).unwrap_or_default();
        if first_cell.contains("交易时间") || first_cell.is_empty() {
            continue;
        }

        let row_str: String = (0..row.len())
            .map(|i| row.get(i).map(|c| c.to_string()).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(" | ");

        if row_str.contains(keyword) {
            let cols: Vec<String> = (0..row.len())
                .map(|i| row.get(i).map(|c| c.to_string()).unwrap_or_default())
                .collect();
            eprintln!("[Row] {:?}", cols);
        }
    }
    Ok(())
}

/// 从状态列中提取退款金额
/// 支持格式："已退款(¥2071.80)"、"已退¥2071.80"、"已退款¥xxx"
pub fn extract_refund_amount(status: &str) -> f64 {
    use regex::Regex;
    // 匹配 "已退款" 或 "已退" 后面的金额，支持括号格式
    let re = Regex::new(r"已退[款]?\s*[\(（]?\s*¥?\s*([\d,.]+)").unwrap();
    if let Some(cap) = re.captures(status) {
        return cap[1].replace(",", "").parse().unwrap_or(0.0);
    }
    0.0
}

/// 从备注列提取优惠金额
/// 支持格式："已优惠¥0.80"、"已优惠0.80"
pub fn extract_discount_amount(remark: &str) -> f64 {
    use regex::Regex;
    let re = Regex::new(r"已优惠\s*¥?\s*([\d,.]+)").unwrap();
    if let Some(cap) = re.captures(remark) {
        return cap[1].replace(",", "").parse().unwrap_or(0.0);
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_file() {
        let result = parse_wechat_bill("nonexistent.xlsx");
        assert!(result.is_err());
    }

    #[test]
    fn debug_parse_real_bill() {
        let path = r"..\data\账单\微信支付账单流水文件(20260418-20260518)_20260518094032.xlsx";
        let records = parse_wechat_bill(path).unwrap();
        // 验证时间已正确解析（不含序列号小数点）
        for r in &records {
            assert!(
                !r.transaction_time.contains('.'),
                "时间仍是序列号: {} for {}",
                r.transaction_time,
                r.merchant_name
            );
        }
        // 验证目标行
        let target = records
            .iter()
            .find(|r| r.merchant_name.contains("华住") && (r.amount - 1104.95).abs() < 1.0);
        assert!(target.is_some(), "未找到华住 1104.95 行");
        let t = target.unwrap();
        assert_eq!(t.transaction_time, "2026-04-24 00:36:40");
        assert_eq!(t.payment_method, "广发银行信用卡(5034)");
        assert!(!records.is_empty());
    }
}
