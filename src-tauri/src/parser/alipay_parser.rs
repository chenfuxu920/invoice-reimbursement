use crate::models::payment::{PaymentRecord, PaymentSource};
use calamine::{open_workbook_auto, Reader};
use uuid::Uuid;

pub fn parse_alipay_bill(file_path: &str) -> Result<Vec<PaymentRecord>, String> {
    let mut workbook = open_workbook_auto(file_path)
        .map_err(|e| format!("打开支付宝账单失败: {}", e))?;

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

        // 支付宝账单标题行
        if first_cell.contains("交易时间") || first_cell.contains("交易号") {
            header_found = true;
            continue;
        }

        if !header_found {
            continue;
        }
        if row.len() < 8 {
            continue;
        }

        let transaction_time = row.get(0).map(|c| c.to_string()).unwrap_or_default();
        let transaction_id = row.get(1).map(|c| c.to_string()).unwrap_or_default();
        let merchant_name = row.get(4).map(|c| c.to_string()).unwrap_or_default();
        let category = row.get(6).map(|c| c.to_string()).unwrap_or_default();
        let amount_str = row.get(8).map(|c| c.to_string()).unwrap_or_default();
        let direction = row.get(9).map(|c| c.to_string()).unwrap_or_default();

        if !direction.contains("支出") {
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
            source: PaymentSource::Alipay,
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
        let result = parse_alipay_bill("nonexistent.xlsx");
        assert!(result.is_err());
    }
}
