//! 单元格引导发票字段提取 — 基于 pdfplumber `find_tables()` 的表格结构，
//! 按行标签定位字段，从相邻值单元格提取具体值。
//!
//! 现有正则/坐标/parangi 方法作为回退（管道在单元格提取后仍会走回退）。

use regex::Regex;

/// 从表格单元格中提取的发票字段
#[derive(Debug, Default)]
pub struct CellInvoiceFields {
    pub seller_name: Option<String>,
    pub amount: Option<f64>,
    /// 税收分类编码简称，`*运输服务*` → "运输服务"
    pub item_name: Option<String>,
    /// 商品详情单元格 bbox，供 pipeline 用 raw_words 做列感知提取 item_detail
    pub item_cell_bbox: Option<(f64, f64, f64, f64)>,
    /// 商品详情单元格的按列聚合文本（Type 4），跨行不丢字，供 pipeline 提取 item_detail
    pub item_cell_text: Option<String>,
    pub remarks: Option<String>,
}

/// 单元格文本变体，供 [`try_cell_texts`] 按序尝试。
#[cfg(feature = "pdfplumber")]
#[derive(Clone, Copy)]
enum CellTextKind {
    /// Type 2: 按行组装（适合横排标签值：销售方信息、价税合计）
    Line,
    /// Type 3: 全合并去空白（适合 *xxx* 编码 / 竖排标签 / 小单元格）
    Merged,
    /// Type 4: 按列聚合（适合商品详情大单元格）
    Column,
    /// Type 0: pdfplumber 原生 text（跨 cell word 归属兜底）
    Raw,
}

/// 值单元格尝试顺序：line → merged → column → raw。
/// line 保空格，`名称：公司名 税号` 类正则依赖空格切分，必须先试。
#[cfg(feature = "pdfplumber")]
const ORDER_VALUE: &[CellTextKind] = &[
    CellTextKind::Line,
    CellTextKind::Merged,
    CellTextKind::Column,
    CellTextKind::Raw,
];

/// 商品名单元格尝试顺序：merged 优先（`*xxx*` 编码去空白后才能匹配），跳过 line
/// （line 的换行/字间空格会让 `\*(.+?)\*` 捕获带空白的名字）。
#[cfg(feature = "pdfplumber")]
const ORDER_ITEM: &[CellTextKind] = &[
    CellTextKind::Merged,
    CellTextKind::Column,
    CellTextKind::Raw,
];

/// 以单元格为主体，对单元格内几种文本变体按 `order` 依次尝试匹配 extractor。
/// 每种变体先经 `remove_cjk_spaces` 清洗，任一命中即返回。
/// 顺序由调用方指定（标签值/商品名需要不同顺序），不做单一固定顺序。
#[cfg(feature = "pdfplumber")]
fn try_cell_texts<T>(
    cell: &TableCellInfo,
    order: &[CellTextKind],
    extractor: &impl Fn(&str) -> Option<T>,
) -> Option<T> {
    for kind in order {
        let text = match kind {
            CellTextKind::Line => remove_cjk_spaces(&cell.line_text),
            CellTextKind::Merged => remove_cjk_spaces(&cell.merged_text),
            CellTextKind::Column => remove_cjk_spaces(&cell.column_text),
            CellTextKind::Raw => remove_cjk_spaces(&cell.text),
        };
        if let Some(v) = extractor(&text) {
            return Some(v);
        }
    }
    None
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
    fields
}

#[cfg(feature = "pdfplumber")]
use crate::pdf::text_extractor::{TableCellInfo, TableInfo};

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

        // 项目名称：搜索含 *xxx* 模式的单元格，记录编码简称、bbox、按列聚合文本
        if fields.item_name.is_none() {
            if let Some((name, bbox, column_text)) = extract_item_from_row(row) {
                fields.item_name = Some(name);
                fields.item_cell_bbox = Some(bbox);
                fields.item_cell_text = Some(column_text);
            }
        }
    }
}

/// 在行中查找标签单元格，从后续单元格中提取值。
/// 标签单元格判定：
/// - contains label 且长度 ≤ 15（竖排标签"销售方信息"等短标签，排除值单元格误匹配）；
/// - 价税合计标签格常含金额大写（"价税合计（大写）贰仟…"或"贰仟…价税合计（大写）"，
///   全电发票合并单元格后大写金额使整格超 15 字符），单独放宽到 60。
/// 值提取优先用 line_text（按行组装，适合横排），line_text 失败则回退 merged_text（全合并，适合小单元格/换行）。
#[cfg(feature = "pdfplumber")]
fn extract_by_label<F, T>(row: &[TableCellInfo], label: &str, extractor: F) -> Option<T>
where
    F: Fn(&str) -> Option<T>,
{
    let label_idx = row.iter().position(|c| {
        let merged_len = c.merged_text.chars().count();
        if c.merged_text.contains(label) && merged_len <= 15 {
            return true;
        }
        // 价税合计标签格含金额大写（可能超 15 字符），放宽；其他标签不放开（防值单元格误匹配）
        if label == "价税合计" && c.merged_text.contains(label) && merged_len <= 60 {
            return true;
        }
        // pdfplumber word 重建可能丢字（竖排标签"销售方信息"→"售方信息"，
        // "销"被 word grouping 并入相邻值列），回退到 char 级原始 text（去空白）匹配
        let raw: String = c.text.chars().filter(|ch| !ch.is_whitespace()).collect();
        let raw_len = raw.chars().count();
        raw.contains(label) && (raw_len <= 15 || (label == "价税合计" && raw_len <= 60))
    })?;

    // 依次尝试标签后的所有单元格（不限于第一个非空单元格）
    for cell in row.iter().skip(label_idx + 1) {
        // 跳过空单元格（line_text 和 merged_text 都空才算空）
        if cell.line_text.trim().is_empty() && cell.merged_text.is_empty() {
            continue;
        }
        // 统一入口：以单元格为主体，按序尝试 line/merged/column/raw 四种文本
        if let Some(v) = try_cell_texts(cell, ORDER_VALUE, &extractor) {
            return Some(v);
        }
    }
    None
}

// ── 值提取函数 ──────────────────────────────────────────

fn extract_seller_value(text: &str) -> Option<String> {
    // 格式0（VAT等）："公司名 名称： 税号..." — 名称在中间，公司名在"名称："前面
    let re_name_before = Regex::new(r"([^\s]{3,40}?)\s*名称[：:]").ok()?;
    if let Some(caps) = re_name_before.captures(text) {
        let name = caps[1].trim();
        if name.chars().count() >= 3
            && !is_tax_id(name)
            && name.chars().any(is_cjk)
            && !name.starts_with("项目")
            && !name.starts_with("货物")
            && !name.starts_with("密码")
        {
            return Some(name.to_string());
        }
    }
    // 格式1（滴滴等）："名称：公司名 税号..."
    let re =
        Regex::new(r"名称[：:]\s*(.+?)(?:\s*称[：:]|\s+[A-Z0-9]{15,}|\s+统一社会|\s+纳税人|$)")
            .ok()?;
    if let Some(caps) = re.captures(text) {
        let name = caps[1].trim();
        if name.chars().count() >= 3 && !is_tax_id(name) {
            return Some(name.to_string());
        }
    }
    // 格式3（CID 交错）："名...前半段公司名...称：...后半段公司名..."
    // VAT 发票 CID 字体间距导致 "名称：" 标签字符与公司名交错排列：
    //   "名长沙市轨称：交通运营有限公司" → 前半段="长沙市轨" + 后半段="交通运营有限公司"
    // 合并前后两段才能得到完整公司名。
    let re_interspersed = Regex::new(r"名(.+?)称[：:](.+)").ok()?;
    if let Some(caps) = re_interspersed.captures(text) {
        let part1 = caps[1].trim();
        let part2: String = caps[2]
            .trim()
            .chars()
            .take_while(|c| !c.is_ascii_digit() && !c.is_whitespace())
            .collect();
        let name = format!("{}{}", part1, part2);
        if name.chars().count() >= 3 && !is_tax_id(&name) && name.chars().any(is_cjk) {
            return Some(name);
        }
        // 合并后太短则尝试只用 part2（"名"后面可能直接是公司名，"称："是残留标签）
        if part2.chars().count() >= 3 && !is_tax_id(&part2) && part2.chars().any(is_cjk) {
            return Some(part2);
        }
        // 只用 part1
        if part1.chars().count() >= 3 && !is_tax_id(part1) && part1.chars().any(is_cjk) {
            return Some(part1.to_string());
        }
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

/// 从行中搜索含 `*xxx*` 的单元格，返回 (税收编码简称, 单元格bbox, 按列聚合文本)
/// 统一入口：merged 优先（`*xxx*` 编码去空白后才可匹配），column 次之，raw 兜底。
/// column_text (Type 4) 按列聚合，跨行不丢字，供 pipeline 提取完整 item_detail
fn extract_item_from_row(row: &[TableCellInfo]) -> Option<(String, (f64, f64, f64, f64), String)> {
    let re = Regex::new(r"\*(.+?)\*").ok()?;
    for cell in row {
        let name = try_cell_texts(cell, ORDER_ITEM, &|text| {
            let caps = re.captures(text)?;
            let name = caps[1].to_string();
            if name.chars().any(|c| is_cjk(c)) {
                Some(name)
            } else {
                None
            }
        });
        if let Some(name) = name {
            let bbox = (cell.x0, cell.top, cell.x1, cell.bottom);
            return Some((name, bbox, cell.column_text.clone()));
        }
    }
    None
}

// ── 文本工具 ────────────────────────────────────────────

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

#[cfg(all(test, feature = "pdfplumber"))]
mod tests {
    use super::*;

    fn cell(text: &str) -> TableCellInfo {
        TableCellInfo {
            text: text.to_string(),
            x0: 0.0,
            top: 0.0,
            x1: 100.0,
            bottom: 20.0,
            words: vec![],
            line_text: text.to_string(),
            merged_text: text.chars().filter(|c| !c.is_whitespace()).collect(),
            column_text: text.to_string(),
        }
    }

    // 统一入口：单元格内几种文本按序尝试，任一命中即返回
    #[test]
    fn try_cell_texts_tries_all_variants() {
        let mut c = cell("名称： 甲公司");
        c.merged_text = "名称：甲公司".to_string();
        let got = try_cell_texts(&c, ORDER_VALUE, &|t| {
            if t.contains("甲公司") {
                Some("hit")
            } else {
                None
            }
        });
        assert_eq!(got, Some("hit"));
    }

    // 统一入口：line 不命中时继续试 merged/column/raw
    #[test]
    fn try_cell_texts_falls_through() {
        let c = cell("no value here");
        let got = try_cell_texts(&c, ORDER_VALUE, &|t| {
            if t.contains("税号") {
                Some(t.to_string())
            } else {
                None
            }
        });
        assert_eq!(got, None);
    }

    // 统一入口：商品名 *xxx* 用 merged 优先（line 的换行会阻断匹配）
    #[test]
    fn try_cell_texts_item_merged_first() {
        let mut c = cell("*运输\n服务*");
        c.merged_text = "*运输服务*".to_string();
        let re = Regex::new(r"\*(.+?)\*").unwrap();
        let got = try_cell_texts(&c, ORDER_ITEM, &|t| {
            re.captures(t).map(|m| m[1].to_string())
        });
        assert_eq!(got.as_deref(), Some("运输服务"));
    }
}
