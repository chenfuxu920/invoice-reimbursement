use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    Amount,
    Date,
    InvoiceNumber,
    SellerName,
    ItemName,
}

/// 根据字段类型和用户拖选的文本，生成正则表达式骨架
pub fn generate_regex(field_type: FieldType, selected_text: &str) -> String {
    match field_type {
        FieldType::Amount => generate_amount_regex(selected_text),
        FieldType::Date => generate_date_regex(selected_text),
        FieldType::InvoiceNumber => generate_invoice_number_regex(selected_text),
        FieldType::SellerName => generate_seller_name_regex(selected_text),
        FieldType::ItemName => generate_item_name_regex(selected_text),
    }
}

/// 泛化冒号：全角/半角 → [：:]
fn generalize_colon(s: &str) -> String {
    if s.contains('：') || s.contains(':') {
        s.replace('：', "[：:]").replace(':', "[：:]")
    } else {
        s.to_string()
    }
}

/// 泛化货币符号：¥/￥ → [￥¥]
fn generalize_currency(s: &str) -> String {
    s.replace('¥', "[￥¥]").replace('￥', "[￥¥]")
}

/// 提取数字前的前缀文本（到第一个数字为止）
fn extract_prefix_before_number(text: &str) -> String {
    let pos = text.find(|c: char| c.is_ascii_digit());
    match pos {
        Some(p) => text[..p].to_string(),
        None => text.to_string(),
    }
}

fn generate_amount_regex(selected: &str) -> String {
    let prefix = extract_prefix_before_number(selected);
    let number_group = "([\\d,]+\\.?\\d*)";

    if prefix.is_empty() {
        return number_group.to_string();
    }

    // 泛化前缀中的冒号和货币符号
    let prefix = generalize_currency(&prefix);
    let prefix = generalize_colon(&prefix);
    // 去除尾部空白，用 \s* 连接
    let prefix = prefix.trim_end();
    format!("{}\\s*{}", prefix, number_group)
}

fn generate_date_regex(selected: &str) -> String {
    if selected.contains('年') || selected.contains('月') || selected.contains('日') {
        r"(\d{4}\s*年\s*\d{1,2}\s*月\s*\d{1,2}\s*日?)".to_string()
    } else if selected.contains('-') || selected.contains('/') {
        r"(\d{4}[-/]\d{1,2}[-/]\d{1,2})".to_string()
    } else {
        r"(\d{4}\s*年\s*\d{1,2}\s*月\s*\d{1,2}\s*日?)".to_string()
    }
}

fn generate_invoice_number_regex(selected: &str) -> String {
    let prefix = extract_prefix_before_number(selected);
    let number_group = "(\\d{8,20})";

    if prefix.is_empty() {
        return number_group.to_string();
    }

    let prefix = generalize_colon(&prefix);
    let prefix = prefix.trim_end();
    format!("{}\\s*{}", prefix, number_group)
}

fn generate_seller_name_regex(selected: &str) -> String {
    // 检测前缀关键词
    let prefixes = ["名称", "销售方", "收款单位", "开票方"];
    for prefix in &prefixes {
        if selected.contains(prefix) {
            let after_prefix = &selected[selected.find(prefix).unwrap() + prefix.len()..];
            // 泛化冒号
            let colon = if after_prefix.starts_with('：') || after_prefix.starts_with(':') {
                "[：:]"
            } else {
                ""
            };
            return format!("{}{}\\s*(.+?)(?:\\s|$)", prefix, colon);
        }
    }
    // 无前缀，匹配整段
    "(.+?)(?:\\s|$)".to_string()
}

fn generate_item_name_regex(selected: &str) -> String {
    if selected.contains('*') {
        return r"\*(.+?)\*".to_string();
    }

    let prefixes = ["项目名称", "货物或应税劳务", "商品名称", "服务名称", "品目"];
    for prefix in &prefixes {
        if selected.contains(prefix) {
            let after_prefix = &selected[selected.find(prefix).unwrap() + prefix.len()..];
            let colon = if after_prefix.starts_with('：') || after_prefix.starts_with(':') {
                "[：:]"
            } else {
                ""
            };
            return format!("{}{}\\s*(.+)", prefix, colon);
        }
    }

    "(.+)".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_with_currency_prefix() {
        let regex = generate_regex(FieldType::Amount, "价税合计：¥1,234.56");
        assert!(regex.contains("价税合计"), "应保留前缀: {}", regex);
        assert!(regex.contains("[\\d,]+\\.?\\d*"), "应包含数字捕获组: {}", regex);
        assert!(regex.contains("("), "应有捕获组: {}", regex);
    }

    #[test]
    fn test_amount_with_yuan_symbol() {
        let regex = generate_regex(FieldType::Amount, "合计：￥500.00");
        assert!(regex.contains("[￥¥]"), "应泛化货币符号: {}", regex);
        assert!(regex.contains("[\\d,]+\\.?\\d*"), "{}", regex);
    }

    #[test]
    fn test_amount_pure_number() {
        let regex = generate_regex(FieldType::Amount, "1234.56");
        assert!(regex.contains("[\\d,]+\\.?\\d*"), "{}", regex);
        assert!(regex.contains("("), "{}", regex);
    }

    #[test]
    fn test_date_chinese_format() {
        let regex = generate_regex(FieldType::Date, "2024年05月20日");
        assert!(regex.contains("\\d{4}"), "应匹配年份: {}", regex);
        assert!(regex.contains("\\d{1,2}"), "应匹配月日: {}", regex);
    }

    #[test]
    fn test_date_iso_format() {
        let regex = generate_regex(FieldType::Date, "2024-05-20");
        assert!(regex.contains("\\d{4}"), "{}", regex);
        assert!(regex.contains("\\d{1,2}"), "{}", regex);
    }

    #[test]
    fn test_invoice_number_with_prefix() {
        let regex = generate_regex(FieldType::InvoiceNumber, "发票号码：12345678");
        assert!(regex.contains("发票号码"), "应保留前缀: {}", regex);
        assert!(regex.contains("\\d{8,20}"), "应匹配数字: {}", regex);
    }

    #[test]
    fn test_seller_name_with_prefix() {
        let regex = generate_regex(FieldType::SellerName, "名称：测试餐饮店");
        assert!(regex.contains("名称"), "应保留前缀: {}", regex);
        assert!(regex.contains("[：:]"), "应泛化冒号: {}", regex);
        assert!(regex.contains("("), "应有捕获组: {}", regex);
    }

    #[test]
    fn test_item_name_with_stars() {
        let regex = generate_regex(FieldType::ItemName, "*住宿服务*");
        assert!(regex.contains("\\*"), "应匹配星号: {}", regex);
        assert!(regex.contains("("), "{}", regex);
    }

    #[test]
    fn test_item_name_with_prefix() {
        let regex = generate_regex(FieldType::ItemName, "项目名称：住宿费");
        assert!(regex.contains("项目名称"), "{}", regex);
        assert!(regex.contains("("), "{}", regex);
    }

    #[test]
    fn test_colon_generalization() {
        // 全角冒号应泛化为 [：:]
        let regex = generate_regex(FieldType::SellerName, "名称：测试公司");
        assert!(regex.contains("[：:]"), "应泛化冒号: {}", regex);
    }
}
