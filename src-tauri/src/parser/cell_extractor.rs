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
    pub remarks: Option<String>,
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

        // 项目名称：搜索含 *xxx* 模式的单元格，记录编码简称和bbox供pipeline列感知提取
        if fields.item_name.is_none() {
            if let Some((name, bbox)) = extract_item_from_row(row) {
                fields.item_name = Some(name);
                fields.item_cell_bbox = Some(bbox);
            }
        }
    }
}

/// 在行中查找标签单元格，从后续单元格中提取值。
/// 标签单元格判定：merged_text（去空白后的文本）包含 label 且长度 ≤ 15（排除值单元格误匹配）。
/// 值提取优先用 line_text（按行组装，适合横排），line_text 失败则回退 merged_text（全合并，适合小单元格/换行）。
#[cfg(feature = "pdfplumber")]
fn extract_by_label<F, T>(row: &[TableCellInfo], label: &str, extractor: F) -> Option<T>
where
    F: Fn(&str) -> Option<T>,
{
    let label_idx = row.iter().position(|c| {
        c.merged_text.contains(label) && c.merged_text.chars().count() <= 15
    })?;
    eprintln!("  [LBLDBG] found label='{label}' at idx={label_idx}, merged='{}/{}'", row[label_idx].merged_text, row[label_idx].merged_text.chars().count());

    // 依次尝试标签后的所有单元格（不限于第一个非空单元格）
    for (si, cell) in row.iter().skip(label_idx + 1).enumerate() {
        // 跳过空单元格（line_text 和 merged_text 都空才算空）
        if cell.line_text.trim().is_empty() && cell.merged_text.is_empty() {
            continue;
        }
        // Type 2: 按行组装文本（适合横排标签值：销售方信息、价税合计）
        let line_cleaned = remove_cjk_spaces(&cell.line_text);
        eprintln!("  [LBLDBG]   skip={si} line_text='{}'", line_cleaned.chars().take(120).collect::<String>());
        if let Some(v) = extractor(&line_cleaned) {
            return Some(v);
        }
        // Type 3: 全合并文本（适合小单元格/换行：行程单等）
        let merged_cleaned = remove_cjk_spaces(&cell.merged_text);
        if let Some(v) = extractor(&merged_cleaned) {
            return Some(v);
        }
        // Type 4: 按列聚合文本（适合单元格内多列布置：商品详情、多列值）
        let column_cleaned = remove_cjk_spaces(&cell.column_text);
        if let Some(v) = extractor(&column_cleaned) {
            return Some(v);
        }
        // Type 0 回退：pdfplumber 原始 text
        let raw_cleaned = remove_cjk_spaces(&cell.text);
        if let Some(v) = extractor(&raw_cleaned) {
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
            && !name.starts_with("项目") && !name.starts_with("货物") && !name.starts_with("密码")
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
    // TEMP: dump codepoints around "称" to debug colon char
    if let Some(pos) = text.find('称') {
        let around: Vec<u32> = text[pos..].char_indices().take(5).map(|(_, c)| c as u32).collect();
        eprintln!("  [CPDBG] chars after 称: {:?}", around);
    }
    if let Some(caps) = re_interspersed.captures(text) {
        let part1 = caps[1].trim();
        let part2: String = caps[2].trim().chars()
            .take_while(|c| !c.is_ascii_digit() && !c.is_whitespace())
            .collect();
        eprintln!("  [INTDBG] matched! part1='{}' part2='{}'", part1, &part2[..part2.len().min(40)]);
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

/// 从行中搜索含 `*xxx*` 的单元格，返回 (税收编码简称, 单元格bbox)
/// 用 merged_text (Type 3) 匹配：*运 输 服 务* → *运输服务*
fn extract_item_from_row(row: &[TableCellInfo]) -> Option<(String, (f64, f64, f64, f64))> {
    let re = Regex::new(r"\*(.+?)\*").ok()?;
    for cell in row {
        // Type 3: merged_text 去除所有空白，适合 *xxx* 税收编码匹配
        if let Some(caps) = re.captures(&cell.merged_text) {
            let name = caps[1].to_string();
            if name.chars().any(|c| is_cjk(c)) {
                let bbox = (cell.x0, cell.top, cell.x1, cell.bottom);
                return Some((name, bbox));
            }
        }
        // Type 4: column_text 按列聚合，适合大单元格内多列项目详情
        let column_cleaned = remove_cjk_spaces(&cell.column_text);
        if let Some(caps) = re.captures(&column_cleaned) {
            let name = caps[1].to_string();
            if name.chars().any(|c| is_cjk(c)) {
                let bbox = (cell.x0, cell.top, cell.x1, cell.bottom);
                return Some((name, bbox));
            }
        }
        // Fallback: pdfplumber 原始 text + remove_cjk_spaces
        let cleaned = remove_cjk_spaces(&cell.text);
        if let Some(caps) = re.captures(&cleaned) {
            let name = caps[1].to_string();
            if name.chars().any(|c| is_cjk(c)) {
                let bbox = (cell.x0, cell.top, cell.x1, cell.bottom);
                return Some((name, bbox));
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
