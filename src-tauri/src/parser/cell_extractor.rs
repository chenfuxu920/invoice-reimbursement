//! 单元格引导发票字段提取 — 基于 pdfplumber `find_tables()` 的表格结构，
//! 按行标签定位字段，从相邻值单元格提取具体值。
//!
//! 现有正则/坐标/parangi 方法作为回退（管道在单元格提取后仍会走回退）。

use crate::models::invoice::HotelDetail;
use chrono::NaiveDate;
use regex::Regex;

/// 从表格单元格中提取的发票字段（全部可选，提取失败则为 None）
#[derive(Debug, Default)]
pub struct CellInvoiceFields {
    pub seller_name: Option<String>,
    pub amount: Option<f64>,
    pub item_name: Option<String>,
    pub remarks: Option<String>,
    pub hotel_detail: Option<HotelDetail>,
}

/// 从 pdfplumber 表格提取发票字段。按页遍历所有表格的所有行，
/// 按行标签匹配字段类型（"销售方"/"价税合计"/"备注"/"项目名称"），
/// 从标签旁的值单元格提取具体值。
#[cfg(feature = "pdfplumber")]
pub fn extract_fields_from_tables(
    tables_by_page: &[Vec<crate::pdf::text_extractor::TableInfo>],
) -> CellInvoiceFields {
    let mut fields = CellInvoiceFields::default();
    for tables in tables_by_page {
        for table in tables {
            extract_fields_from_table(table, &mut fields);
        }
    }
    // 从备注解析酒店详情
    if let Some(ref remarks) = fields.remarks {
        if let Some(hd) = crate::parser::invoice_parser::parse_hotel_detail(
            remarks,
            NaiveDate::default(),
        ) {
            fields.hotel_detail = Some(hd);
        }
    }
    fields
}

#[cfg(feature = "pdfplumber")]
use crate::pdf::text_extractor::{TableInfo, TableCellInfo};

#[cfg(feature = "pdfplumber")]
fn extract_fields_from_table(table: &TableInfo, fields: &mut CellInvoiceFields) {
    for row in &table.rows {
        if row.is_empty() {
            continue;
        }

        // 销售方：找"销售方"标签单元格 → 下一非空单元格为值
        if fields.seller_name.is_none() {
            if let Some(v) = extract_by_label(row, "销售方", extract_seller_value) {
                fields.seller_name = Some(v);
            }
        }

        // 金额：找"价税合计"标签单元格 → 下一非空单元格为值
        if fields.amount.is_none() {
            if let Some(v) = extract_by_label(row, "价税合计", extract_amount_value) {
                fields.amount = Some(v);
            }
        }

        // 备注：找"备注"标签单元格 → 下一非空单元格为值
        if fields.remarks.is_none() {
            if let Some(v) = extract_by_label(row, "备注", extract_remarks_value) {
                // ponytail: 跳过过短值（旧版发票同行有"备注"标签但值是序号"3"等）
                if v.chars().count() >= 3 {
                    fields.remarks = Some(v);
                }
            }
        }

        // 项目名称：搜索含 *xxx* 模式的单元格
        if fields.item_name.is_none() {
            if let Some(v) = extract_item_from_row(row) {
                fields.item_name = Some(v);
            }
        }
    }
}

/// 在行中查找标签单元格，从下一非空单元格提取值。
/// 标签单元格判定：去空格后包含 label 且长度 ≤ 15（排除值单元格误匹配）。
#[cfg(feature = "pdfplumber")]
fn extract_by_label<F, T>(row: &[TableCellInfo], label: &str, extractor: F) -> Option<T>
where
    F: Fn(&str) -> Option<T>,
{
    let label_idx = row.iter().position(|c| {
        let normalized = normalize_label(&c.text);
        normalized.contains(label) && normalized.chars().count() <= 15
    })?;

    // 值单元格 = 标签后的下一个非空单元格
    let value_cell = row
        .iter()
        .skip(label_idx + 1)
        .find(|c| !c.text.trim().is_empty())?;

    let cleaned = remove_cjk_spaces(&value_cell.text);
    extractor(&cleaned)
}

// ── 值提取函数 ──────────────────────────────────────────

fn extract_seller_value(text: &str) -> Option<String> {
    // 格式1（滴滴等）："名称：公司名 税号..."
    // 格式2（VAT等）："公司名 名称： 税号..."
    // 先尝试名称后提取
    let re =
        Regex::new(r"名称[：:]\s*(.+?)(?:\s*称[：:]|\s+[A-Z0-9]{15,}|\s+统一社会|\s+纳税人|$)")
            .ok()?;
    if let Some(caps) = re.captures(text) {
        let name = caps[1].trim();
        if name.chars().count() >= 3 && !is_tax_id(name) {
            return Some(name.to_string());
        }
    }
    // 格式3（旧版043字符交错）："名公司名称: 税号..."（"名"和"称"被公司名隔开）
    let re_jumbled = Regex::new(r"名(.+?)称[：:]").ok()?;
    if let Some(caps) = re_jumbled.captures(text) {
        let name = caps[1].trim();
        if name.chars().count() >= 3 && !is_tax_id(name) {
            return Some(name.to_string());
        }
    }
    // 格式2：公司名在"名称："之前 — 用公司名后缀模式提取
    if let Some(name) = crate::parser::invoice_parser::extract_company_name_fallback(text) {
        return Some(name);
    }
    // 回退：复用 invoice_parser 的 extract_seller_name
    let seller = crate::parser::invoice_parser::extract_seller_name(text);
    if seller.is_empty() {
        None
    } else {
        Some(seller)
    }
}

/// 判断文本是否是税号（纯字母数字、长度≥15）
fn is_tax_id(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.len() >= 15 && trimmed.chars().all(|c| c.is_ascii_alphanumeric())
}

fn extract_amount_value(text: &str) -> Option<f64> {
    // 单元格已定位到金额值，直接找两位小数
    let re = Regex::new(r"(\d[\d,]*\.\d{2})").ok()?;
    for cap in re.captures_iter(text) {
        let v: f64 = cap[1].replace(",", "").parse().ok()?;
        if v > 0.0 && v < 1_000_000.0 {
            return Some(v);
        }
    }
    None
}

fn extract_remarks_value(text: &str) -> Option<String> {
    let cleaned = text.trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn extract_item_from_row(row: &[TableCellInfo]) -> Option<String> {
    for cell in row {
        let cleaned = remove_cjk_spaces(&cell.text);
        let re = Regex::new(r"\*(.+?)\*").ok()?;
        if let Some(caps) = re.captures(&cleaned) {
            let name = caps[1].to_string();
            // ponytail: 排除密码区乱码（*xxx* 内容必须含 CJK 字符才是服务类型名）
            if name.chars().any(|c| is_cjk(c)) {
                return Some(name);
            }
        }
    }
    None
}

// ── 文本工具 ────────────────────────────────────────────

/// 去除所有空白字符，用于标签匹配（"销 售 方 信 息" → "销售方信息"）
fn normalize_label(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// 去除 CJK 字符间的空格（旧版发票 pdfplumber 输出字符间有空格："名 长 沙" → "长沙"）
/// 保留 CJK 与非 CJK 间的空格（"公司 91430" → "公司 91430"）
fn remove_cjk_spaces(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::new();
    for i in 0..chars.len() {
        if chars[i] == ' ' && i > 0 && i + 1 < chars.len() {
            if is_cjk(chars[i - 1]) && is_cjk(chars[i + 1]) {
                continue;
            }
        }
        result.push(chars[i]);
    }
    result
}

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3000}'..='\u{303f}' | '\u{ff00}'..='\u{ffef}')
}
