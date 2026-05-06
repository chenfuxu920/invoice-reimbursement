use crate::models::payment::{PaymentRecord, PaymentSource};
use calamine::{open_workbook_auto, Reader};
use uuid::Uuid;

pub fn parse_wechat_bill(file_path: &str) -> Result<Vec<PaymentRecord>, String> {
    let mut workbook = open_workbook_auto(file_path)
        .map_err(|e| format!("打开微信账单失败: {}", e))?;

    let sheet = workbook
        .sheet_names()
        .get(0)
        .ok_or("无工作表")?
        .clone();

    let range = workbook
        .worksheet_range(&sheet)
        .map_err(|e| format!("读取工作表失败: {}", e))?;

    let mut records = Vec::new();
    let mut header_found = false;

    for row in range.rows() {
        let first_cell = row.get(0).map(|c| c.to_string()).unwrap_or_default();

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

        let transaction_time = row.get(0).map(|c| c.to_string()).unwrap_or_default();
        let merchant_name = row.get(2).map(|c| c.to_string()).unwrap_or_default();
        let category = row.get(3).map(|c| c.to_string()).unwrap_or_default();
        let direction = row.get(4).map(|c| c.to_string()).unwrap_or_default();
        let amount_str = row.get(5).map(|c| c.to_string()).unwrap_or_default();
        let transaction_id = row.get(8).map(|c| c.to_string()).unwrap_or_default();

        // 只取支出记录
        if !direction.contains("支") {
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

        records.push(PaymentRecord {
            id: Uuid::new_v4().to_string(),
            transaction_id,
            transaction_time,
            amount,
            merchant_name,
            source: PaymentSource::Wechat,
            category,
        });
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_file() {
        // 无真实文件时，测试函数签名和类型
        let result = parse_wechat_bill("nonexistent.xlsx");
        assert!(result.is_err());
    }
}
