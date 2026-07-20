//! Layout extractor — column detection, region extraction, and seller coordinate
//! extraction operating directly on raw pdfplumber `Word` coordinates (before
//! line-merging), preserving multi-column invoice layout.
//!
//! All functionality is gated behind `#[cfg(feature = "pdfplumber")]`.

use pdfplumber::{BBox, Word};

use crate::ocr::engine::bbox_to_json;
use crate::ocr::OcrTextItem;

// ── Layout Tuning ──────────────────────────────────────────────────────

/// 布局提取的可调参数集合，集中管理原先散布在多个函数中的硬编码值。
///
/// 所有阈值都从字体高度（avg_height）派生，确保对不同字号的自适应性。
/// 通过 `LayoutTuning::default()` 获取经过实战验证的默认值。
///
/// # 参数说明
///
/// | 参数 | 默认值 | 说明 |
/// |------|--------|------|
/// | `y_tolerance_pct` | 0.5 | Y 容差 = avg_height × 此值，控制同行判定 |
/// | `x_gap_threshold_pct` | 2.0 | X 间距阈值 = avg_height × 此值，控制同组判定 |
/// | `default_avg_height` | 12.0 | avg_height 为零时的回退值 |
/// | `header_y_threshold` | 80.0 | 表头 Y 排除阈值（PDF 点值），低于此 Y 的词不参与列检测 |
/// | `column_gap_bins` | 3 | 列间隙检测：≥此数量的连续空桶判定为列间隙 |
/// | `bucket_width_pct` | 0.6 | 直方图桶宽 = avg_height × 此值 |
/// | `bucket_width_floor` | 8.0 | 桶宽下限，防止过窄桶 |
/// | `amount_band_top_offset` | 5.0 | 金额 Y 带上边界 = 锚点 top - 此值 |
/// | `amount_band_bottom_offset` | 30.0 | 金额 Y 带下边界 = 锚点 bottom + 此值 |
/// | `max_valid_amount` | 1_000_000.0 | 最大有效金额（排除税号等长数字） |
/// | `min_seller_chars` | 3 | 最小卖方名称字符数 |
/// | `garble_threshold` | 0.3 | CID 乱码检测阈值（乱码字符占比） |
#[derive(Debug, Clone)]
pub struct LayoutTuning {
    pub y_tolerance_pct: f64,
    pub x_gap_threshold_pct: f64,
    pub default_avg_height: f64,
    pub header_y_threshold: f64,
    pub column_gap_bins: usize,
    pub bucket_width_pct: f64,
    pub bucket_width_floor: f64,
    pub amount_band_top_offset: f64,
    pub amount_band_bottom_offset: f64,
    pub max_valid_amount: f64,
    pub min_seller_chars: usize,
    pub garble_threshold: f64,
}

impl Default for LayoutTuning {
    fn default() -> Self {
        Self {
            y_tolerance_pct: 0.5,
            x_gap_threshold_pct: 2.0,
            default_avg_height: 12.0,
            header_y_threshold: 80.0,
            column_gap_bins: 3,
            bucket_width_pct: 0.6,
            bucket_width_floor: 8.0,
            amount_band_top_offset: 5.0,
            amount_band_bottom_offset: 30.0,
            max_valid_amount: 1_000_000.0,
            min_seller_chars: 3,
            garble_threshold: 0.3,
        }
    }
}

impl LayoutTuning {
    /// 从 Word 列表计算平均字高
    pub fn avg_height_of(words: &[Word]) -> f64 {
        if words.is_empty() {
            return 12.0;
        }
        let sum: f64 = words.iter().map(|w| w.bbox.height()).sum();
        let count = words.len() as f64;
        if count > 0.0 && sum > 0.0 {
            sum / count
        } else {
            12.0
        }
    }

    /// 计算 Y 容差 = avg_height × y_tolerance_pct
    pub fn y_tolerance(&self, avg_height: f64) -> f64 {
        avg_height * self.y_tolerance_pct
    }

    /// 计算 X 间距阈值 = avg_height × x_gap_threshold_pct
    pub fn x_gap_threshold(&self, avg_height: f64) -> f64 {
        avg_height * self.x_gap_threshold_pct
    }

    /// 计算直方图桶宽 = max(avg_height × bucket_width_pct, bucket_width_floor)
    pub fn bucket_width(&self, avg_height: f64) -> f64 {
        (avg_height * self.bucket_width_pct).max(self.bucket_width_floor)
    }
}

// ── Data Structures ────────────────────────────────────────────────────

/// Label identifying which side of a multi-column layout a column occupies.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnLabel {
    Left,
    Right,
    Full,
}

/// A single column defined by its horizontal extent and label.
#[derive(Debug, Clone)]
pub struct Column {
    pub x_min: f64,
    pub x_max: f64,
    pub label: ColumnLabel,
}

/// 检测到的竖排标题（如"销售方信息"/"购买方信息"/"价税合计"/"备注"）
#[derive(Debug, Clone)]
pub struct VerticalTitle {
    pub text: String,           // 合并后的完整标题文本，如 "销售方信息"
    pub x_min: f64,             // 标题 X 列左边界
    pub x_max: f64,             // 标题 X 列右边界
    pub y_min: f64,             // 标题 Y 起始（顶部）
    pub y_max: f64,             // 标题 Y 结束（底部）
    pub title_type: VerticalTitleType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalTitleType {
    Seller,   // "销售方信息" / "销售方"
    Buyer,    // "购买方信息" / "购买方"
    Total,    // "价税合计"
    Remarks,  // "备注"
}

/// Describes the columnar layout of a page based on word X-coordinate gaps.
#[derive(Debug, Clone)]
pub struct ColumnarLayout {
    pub columns: Vec<Column>,
}

impl ColumnarLayout {
    /// Returns `true` when the page is a single column (no column gap detected).
    pub fn is_single_column(&self) -> bool {
        self.columns.len() == 1
    }

    /// Returns the right column, if present.
    pub fn right_column(&self) -> Option<&Column> {
        self.columns.iter().find(|c| c.label == ColumnLabel::Right)
    }

    /// Returns the left column, if present.
    pub fn left_column(&self) -> Option<&Column> {
        self.columns.iter().find(|c| c.label == ColumnLabel::Left)
    }
}

// ── Column Detection ───────────────────────────────────────────────────

/// Detect column layout from raw pdfplumber Words using X-coordinate histogram gap analysis.
///
/// Algorithm:
/// 1. Exclude header words (`top < header_y_threshold`).
/// 2. Compute average word height → derive bucket width (`max(avg_height × bucket_width_pct, bucket_width_floor)`).
/// 3. Bin body words by X, find runs of ≥`column_gap_bins` consecutive empty bins as column gaps.
/// 4. Take the widest gap, split at its center → left/right columns.
/// 5. No gap → single `Full` column.
pub fn detect_columns(words: &[Word]) -> ColumnarLayout {
    detect_columns_with_tuning(words, &LayoutTuning::default())
}

/// 带自定义参数的列检测（供测试和未来配置化使用）
pub fn detect_columns_with_tuning(words: &[Word], tuning: &LayoutTuning) -> ColumnarLayout {
    if words.is_empty() {
        return ColumnarLayout { columns: vec![] };
    }

    // Exclude header words (Y < threshold)
    let body_words: Vec<&Word> = words.iter().filter(|w| w.bbox.top >= tuning.header_y_threshold).collect();
    if body_words.is_empty() {
        return ColumnarLayout { columns: vec![] };
    }

    // Compute average word height for adaptive bucket sizing
    let sum_h: f64 = body_words.iter().map(|w| w.bbox.height()).sum();
    let avg_height = if body_words.is_empty() { tuning.default_avg_height } else { sum_h / body_words.len() as f64 };
    let bucket_width = tuning.bucket_width(avg_height);

    // Determine X range
    let min_x = body_words
        .iter()
        .map(|w| w.bbox.x0)
        .fold(f64::INFINITY, f64::min);
    let max_x = body_words
        .iter()
        .map(|w| w.bbox.x1)
        .fold(f64::NEG_INFINITY, f64::max);
    let range = max_x - min_x;

    // If the entire content fits in a single bucket, it's single-column
    if range <= bucket_width {
        return ColumnarLayout {
            columns: vec![Column {
                x_min: min_x,
                x_max: max_x,
                label: ColumnLabel::Full,
            }],
        };
    }

    let num_bins = ((range / bucket_width).ceil() as usize).max(1);
    let mut bins: Vec<bool> = vec![false; num_bins];

    for w in &body_words {
        let idx = ((w.bbox.x0 - min_x) / bucket_width).floor() as usize;
        if idx < num_bins {
            bins[idx] = true;
        }
    }

    // Find runs of consecutive empty bins (gaps)
    let gap_threshold = tuning.column_gap_bins;
    let mut gaps: Vec<(usize, usize)> = Vec::new(); // (start, end) inclusive
    let mut i = 0;
    while i < num_bins {
        if !bins[i] {
            let start = i;
            while i < num_bins && !bins[i] {
                i += 1;
            }
            let end = i - 1;
            if end - start + 1 >= gap_threshold {
                gaps.push((start, end));
            }
        } else {
            i += 1;
        }
    }

    if gaps.is_empty() {
        return ColumnarLayout {
            columns: vec![Column {
                x_min: min_x,
                x_max: max_x,
                label: ColumnLabel::Full,
            }],
        };
    }

    // Take the widest gap and split at its center
    let widest = gaps
        .iter()
        .max_by_key(|(s, e)| *e - *s)
        .expect("at least one gap");
    let split_bin = (widest.0 + widest.1) as f64 / 2.0;
    let split_x = min_x + split_bin * bucket_width;

    ColumnarLayout {
        columns: vec![
            Column {
                x_min: min_x,
                x_max: split_x,
                label: ColumnLabel::Left,
            },
            Column {
                x_min: split_x,
                x_max: max_x,
                label: ColumnLabel::Right,
            },
        ],
    }
}

// ── Vertical Title Detection ───────────────────────────────────────────

/// 竖排标题模式列表（长模式优先，避免"销售方"提前匹配"销售方信息"）
const VERTICAL_TITLE_PATTERNS: &[(&str, VerticalTitleType)] = &[
    ("销售方信息", VerticalTitleType::Seller),
    ("购买方信息", VerticalTitleType::Buyer),
    ("价税合计",   VerticalTitleType::Total),
    ("销售方",     VerticalTitleType::Seller),
    ("购买方",     VerticalTitleType::Buyer),
    ("备注",       VerticalTitleType::Remarks),
];

/// 内部平坦化坐标结构，用于竖排标题检测统一处理 Word / OcrTextItem
struct FlatItem {
    text: String,
    x_center: f64,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

/// 从平坦化坐标列表检测竖排标题的共享实现
fn detect_vertical_titles_inner(flat_items: Vec<FlatItem>, avg_height: f64) -> Vec<VerticalTitle> {
    if flat_items.is_empty() || avg_height <= 0.0 {
        return vec![];
    }

    let x_tol = avg_height.max(6.0) * 0.5;

    // ── 1. Group by X-center ────────────────────────────────────────────
    let mut groups: Vec<Vec<FlatItem>> = Vec::new();
    'outer: for item in flat_items {
        for group in groups.iter_mut() {
            let group_x = group.iter().map(|i| i.x_center).sum::<f64>() / group.len() as f64;
            if (item.x_center - group_x).abs() <= x_tol {
                group.push(item);
                continue 'outer;
            }
        }
        groups.push(vec![item]);
    }

    // ── 2. Each group: sort by Y, concatenate, check pattern ────────────
    let mut titles: Vec<VerticalTitle> = Vec::new();

    for group in groups.iter_mut() {
        group.sort_by(|a, b| a.y_min.partial_cmp(&b.y_min).unwrap());

        let concatenated: String = group.iter().map(|i| i.text.as_str()).collect();

        // 验证规则：
        //   - 场景A（pdfplumber 拆字）：每个 word 是单字/短片段（≤3 字符），
        //     组内 word 数 ≥ 标题字符数 × 0.8
        //   - 场景B（parangi 已合并）：单个 word 直接等于完整标题
        let is_merged_single_word = group.len() == 1
            && VERTICAL_TITLE_PATTERNS
                .iter()
                .any(|(p, _)| group[0].text.contains(*p));
        let all_short = group.iter().all(|i| i.text.chars().count() <= 3);

        if !all_short && !is_merged_single_word {
            continue;
        }

        for &(pattern, title_type) in VERTICAL_TITLE_PATTERNS {
            if concatenated.contains(pattern) {
                // 验证：场景A 需要足量 word，场景B 无需（1 word 已含完整标题）
                if all_short {
                    let min_words = (pattern.chars().count() as f64 * 0.8).ceil() as usize;
                    if group.len() < min_words {
                        continue;
                    }
                }

                // 计算该组坐标边界（取所有 word 的并集）
                let x_min = group.iter().map(|i| i.x_min).fold(f64::INFINITY, f64::min);
                let x_max = group.iter().map(|i| i.x_max).fold(f64::NEG_INFINITY, f64::max);
                let y_min = group.iter().map(|i| i.y_min).fold(f64::INFINITY, f64::min);
                let y_max = group.iter().map(|i| i.y_max).fold(f64::NEG_INFINITY, f64::max);

                titles.push(VerticalTitle {
                    text: pattern.to_string(),
                    x_min,
                    x_max,
                    y_min,
                    y_max,
                    title_type,
                });
                break; // 每组只匹配一个模式（长优先）
            }
        }
    }

    // ── 3. Sort by Y ───────────────────────────────────────────────────
    titles.sort_by(|a, b| a.y_min.partial_cmp(&b.y_min).unwrap());
    titles
}

/// 从 pdfplumber Word 列表中检测竖排标题。
///
/// 算法：
/// 1. 按 X 中心分组（容差 = avg_height × 0.5），同 X 列的 word 归为一组
/// 2. 每组内按 Y 排序后拼接文本，检查是否包含已知标题模式
/// 3. 验证：每组 word 均为短片段（≤3 字符）且数量 ≥ 标题字符数 × 0.8
/// 4. 按 y_min 排序输出
pub fn detect_vertical_titles_from_words(words: &[Word]) -> Vec<VerticalTitle> {
    let avg_height = LayoutTuning::avg_height_of(words);

    let flat: Vec<FlatItem> = words
        .iter()
        .map(|w| {
            let cx = (w.bbox.x0 + w.bbox.x1) / 2.0;
            FlatItem {
                text: w.text.clone(),
                x_center: cx,
                x_min: w.bbox.x0,
                x_max: w.bbox.x1,
                y_min: w.bbox.top,
                y_max: w.bbox.bottom,
            }
        })
        .collect();

    detect_vertical_titles_inner(flat, avg_height)
}

/// 从 OcrTextItem 列表中检测竖排标题（OCR 回退路径用）。
///
/// 坐标从 `box_coords["points"]` 数组中提取（参考 `invoice_parser.rs:1111-1124`）。
pub fn detect_vertical_titles_from_items(items: &[crate::ocr::OcrTextItem]) -> Vec<VerticalTitle> {
    // 计算平均字高
    let heights: Vec<f64> = items
        .iter()
        .filter_map(|t| {
            let pts = t.box_coords.as_ref()?.get("points")?.as_array()?;
            let ys: Vec<f64> = pts.iter().filter_map(|p| p.get("y").and_then(|v| v.as_f64())).collect();
            if ys.len() < 2 { return None; }
            let y0 = ys.iter().cloned().fold(f64::INFINITY, f64::min);
            let y1 = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            Some(y1 - y0)
        })
        .collect();
    let avg_height = if heights.is_empty() { 12.0 } else {
        heights.iter().sum::<f64>() / heights.len() as f64
    };

    let flat: Vec<FlatItem> = items
        .iter()
        .filter_map(|t| {
            let coords = t.box_coords.as_ref()?;
            let pts = coords.get("points")?.as_array()?;
            let xs: Vec<f64> = pts.iter().filter_map(|p| p.get("x").and_then(|v| v.as_f64())).collect();
            let ys: Vec<f64> = pts.iter().filter_map(|p| p.get("y").and_then(|v| v.as_f64())).collect();
            if xs.is_empty() || ys.is_empty() {
                return None;
            }
            let x_min = xs.iter().cloned().fold(f64::INFINITY, f64::min);
            let x_max = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let y_min = ys.iter().cloned().fold(f64::INFINITY, f64::min);
            let y_max = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let cx = (x_min + x_max) / 2.0;
            Some(FlatItem {
                text: t.text.clone(),
                x_center: cx,
                x_min,
                x_max,
                y_min,
                y_max,
            })
        })
        .collect();

    detect_vertical_titles_inner(flat, avg_height)
}

/// 根据竖排标题坐标提取其右侧/下方的 Word。
///
/// - `RegionDirection::Right`: 收集 `x0 >= title.x_max - 容差` 且在 Y 范围内的 word
/// - `RegionDirection::Below`: 收集 `top >= title.y_max - 容差` 且在 X 范围内的 word
/// - 结果按 Y 再 X 排序
pub fn extract_region_words_by_title<'a>(
    title: &VerticalTitle,
    words: &'a [Word],
    direction: RegionDirection,
) -> Vec<&'a Word> {
    let avg_height = LayoutTuning::avg_height_of(words);
    let tol = avg_height.max(6.0) * 0.5;

    match direction {
        RegionDirection::Right => {
            let mut result: Vec<&Word> = words
                .iter()
                .filter(|w| {
                    w.bbox.x0 >= title.x_max - tol
                        && w.bbox.top >= title.y_min - tol
                        && w.bbox.bottom <= title.y_max + tol
                })
                .collect();
            result.sort_by(|a, b| {
                a.bbox
                    .top
                    .partial_cmp(&b.bbox.top)
                    .unwrap()
                    .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
            });
            result
        }
        RegionDirection::Below => {
            let mut result: Vec<&Word> = words
                .iter()
                .filter(|w| {
                    w.bbox.top >= title.y_max - tol
                        && w.bbox.x0 >= title.x_min - tol
                        && w.bbox.x1 <= title.x_max + tol
                })
                .collect();
            result.sort_by(|a, b| {
                a.bbox
                    .top
                    .partial_cmp(&b.bbox.top)
                    .unwrap()
                    .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
            });
            result
        }
    }
}

/// 竖排标题内容区域方向
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionDirection {
    /// 标题右侧（seller/buyer 内容区）
    Right,
    /// 标题下方（total/remarks 内容区）
    Below,
}

// ── Anchor Word Finding ────────────────────────────────────────────────

/// Find the first word whose `text` contains any of the given `keywords`.
pub fn find_anchor_word<'a>(words: &'a [Word], keywords: &[&str]) -> Option<&'a Word> {
    words
        .iter()
        .find(|w| keywords.iter().any(|k| w.text.contains(*k)))
}

// ── Region Extraction ──────────────────────────────────────────────────

/// Return all words whose center point falls within the given bounding box.
///
/// Center point = `((x0+x1)/2, (top+bottom)/2)`.
pub fn extract_region_words<'a>(
    words: &'a [Word],
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
) -> Vec<&'a Word> {
    words
        .iter()
        .filter(|w| {
            let cx = (w.bbox.x0 + w.bbox.x1) / 2.0;
            let cy = (w.bbox.top + w.bbox.bottom) / 2.0;
            cx >= x_min && cx <= x_max && cy >= y_min && cy <= y_max
        })
        .collect()
}

// ── Seller Extraction by Raw Coordinates ───────────────────────────────

/// Extract the seller (销售方) name from raw pdfplumber Words by taking the
/// rightmost "名称：" word block.
///
/// Returns an empty string if no matching word is found.
///
/// Handles two invoice layout formats:
/// 1. Standard: `名称：[company name]` — company name follows the label
/// 2. Reversed: `[company name]名称：[label]` — company name precedes the label
///    (found in some electronic invoice formats where pdfplumber merges the
///    seller name with trailing labels into one wide Word)
pub fn extract_seller_by_raw_coords(words: &[Word]) -> String {
    // Find all words containing "名称" and either "：" or ":"
    let candidates: Vec<&Word> = words
        .iter()
        .filter(|w| {
            w.text.contains("名称") && (w.text.contains("：") || w.text.contains(":"))
        })
        .collect();

    if candidates.is_empty() {
        return String::new();
    }

    // Take the rightmost candidate by X-center (not X0, which fails on wide
    // pdfplumber words that span both columns — their left edge may be left of
    // the buyer's).  X-center = (x0 + x1) / 2 reliably places right-column
    // sellers to the right of left-column buyers.
    let seller_word = candidates
        .iter()
        .max_by(|a, b| {
            let ax = (a.bbox.x0 + a.bbox.x1) / 2.0;
            let bx = (b.bbox.x0 + b.bbox.x1) / 2.0;
            ax.partial_cmp(&bx).unwrap()
        })
        .expect("candidates non-empty");

    let text = &seller_word.text;
    let extracted = if let Some(pos) = text.find("名称：") {
        let after = clean_seller_name(&text[pos + "名称：".len()..]);
        if after.is_empty() || after.contains("名称") {
            // Text after "名称：" is just a label artifact (e.g. "买", "名称：买").
            // The actual company name may appear BEFORE "名称：" in this word
            // (reversed format: "[company name]名称：[label]").
            extract_company_name_before_label(&text[..pos])
        } else {
            after
        }
    } else if let Some(pos) = text.find("名称:") {
        let after = clean_seller_name(&text[pos + "名称:".len()..]);
        if after.is_empty() || after.contains("名称") {
            extract_company_name_before_label(&text[..pos])
        } else {
            after
        }
    } else {
        String::new()
    };

    // If the extracted name looks like a buyer (contains buyer keywords), the
    // real seller may be a separate company-name word to the RIGHT on the same
    // Y line — some invoice formats (e.g. 天府通) omit the "名称：" prefix for
    // the seller.  Search for a company-name word (contains "公司") on the same
    // Y line with X-center greater than the candidate's.
    if !extracted.is_empty() && is_likely_buyer(&extracted) {
        let cand_center_x = (seller_word.bbox.x0 + seller_word.bbox.x1) / 2.0;
        let cand_top = seller_word.bbox.top;
        let cand_bottom = seller_word.bbox.bottom;
        let y_tol = ((cand_bottom - cand_top) * 0.5).max(3.0);
        if let Some(seller) = words
            .iter()
            .filter(|w| {
                let cx = (w.bbox.x0 + w.bbox.x1) / 2.0;
                let cy = (w.bbox.top + w.bbox.bottom) / 2.0;
                cx > cand_center_x
                    && (cy - (cand_top + cand_bottom) / 2.0).abs() <= y_tol
                    && w.text.contains("公司")
                    && !w.text.contains("名称")
            })
            .max_by(|a, b| {
                let ax = (a.bbox.x0 + a.bbox.x1) / 2.0;
                let bx = (b.bbox.x0 + b.bbox.x1) / 2.0;
                ax.partial_cmp(&bx).unwrap()
            })
        {
            return clean_seller_name(&seller.text);
        }
    }

    extracted
}

/// Check if a name looks like a buyer (purchaser) rather than a seller.
/// 买方关键词列表 — 用于检测"名称："候选是否实际是买方（购买方）。
///
/// 当右栏"名称："提取的名字包含这些关键词时，触发向右搜索真实卖方。
///
/// **注意**：此列表包含机构类关键词（国防/大学/学院/医院），对特定客户有效。
/// 未来应移至用户配置，默认仅保留 "购买方"。
pub const BUYER_KEYWORDS: &[&str] = &["购买方", "国防", "大学", "学院", "医院"];

/// Check if a name looks like a buyer (purchaser) rather than a seller.
/// Used to detect when the rightmost "名称：" candidate is actually the buyer,
/// triggering a search for the real seller to its right.
pub(crate) fn is_likely_buyer(name: &str) -> bool {
    BUYER_KEYWORDS.iter().any(|k| name.contains(k))
}

/// Remove common label artifacts from the extracted seller name.
///
/// Label chars (销/买/售/购/方) are only stripped from the **end** of the
/// string via `trim_end_matches` — removing them globally would corrupt
/// legitimate company names containing e.g. "营销" or "直销".
fn clean_seller_name(raw: &str) -> String {
    raw.trim()
        // Multi-char patterns first — otherwise replacing "售" individually
        // would break "销售方" before it gets a chance to match.
        .replace("销售方", "")
        .replace("销售", "")
        // Strip trailing single-char label artifacts only (销/买/售/购/方).
        .trim_end_matches(|c: char| "销买售购方".contains(c))
        .trim()
        .to_string()
}

/// Extract a company name from text appearing **before** a "名称：" label.
///
/// Used when the text after "名称：" is just a label artifact (e.g. "买",
/// "名称：买"), and the actual company name appears before "名称：" in the
/// same pdfplumber Word. This happens in some electronic invoice formats
/// where pdfplumber merges the seller name with trailing labels into one
/// wide Word, producing text like:
///   `"四川景澜酒店管理有限公司名称：名称：买"`
///
/// Strips leading/trailing label characters (购/买/售/销/方/密) and returns
/// the remaining text if it contains a company suffix ("公司", "酒店", "中心").
/// Returns empty string if no company suffix is found.
fn extract_company_name_before_label(text_before: &str) -> String {
    // Strip common label characters and whitespace
    let cleaned: String = text_before
        .chars()
        .filter(|c| !"购买售销方密 \t\r\n".contains(*c))
        .collect();

    // Require a company suffix to avoid returning random text
    if cleaned.contains("公司")
        || cleaned.contains("酒店")
        || cleaned.contains("中心")
    {
        return cleaned.trim().to_string();
    }

    String::new()
}

// ── Amount Extraction by Coordinates ───────────────────────────────────

/// Use the "价税合计" (total-including-tax) anchor word's coordinates to locate
/// the total-amount region, then extract the largest valid amount value from a
/// compact Y-band that excludes items-table rows (which sit at a higher Y).
///
/// This is more reliable than extracting from pdfplumber's merged lines, because
/// `merge_words_into_lines` can merge the tax-exclusive amount (items table) with
/// the tax-inclusive amount (total area), causing the wrong value to be picked.
///
/// The approach:
/// 1. Find the anchor word ("价税合计" / "合计金额" / "总金额").
/// 2. Define a compact Y-band: anchor.top - 5 to anchor.bottom + 30.
/// 3. Collect all words whose center falls within the band (full X width).
/// 4. Join the region text and extract all valid 2-decimal amounts via regex.
/// 5. Return the maximum amount (< `max_valid_amount` to exclude tax IDs).
///
/// Returns `None` if no anchor is found or no valid amount exists in the region.
pub fn extract_amount_by_coords(words: &[Word]) -> Option<f64> {
    extract_amount_by_coords_with_tuning(words, &LayoutTuning::default())
}

/// 带自定义参数的金额坐标提取（供测试和未来配置化使用）
pub fn extract_amount_by_coords_with_tuning(words: &[Word], tuning: &LayoutTuning) -> Option<f64> {
    // 1. Find anchor word
    let anchor = find_anchor_word(words, &["价税合计", "合计金额", "总金额"])?;

    // 2. Define compact Y-band (使用 LayoutTuning 的偏移量)
    let page_max_x = words
        .iter()
        .map(|w| w.bbox.x1)
        .fold(0.0f64, f64::max);
    let region_words = extract_region_words(
        words,
        0.0,
        anchor.bbox.top - tuning.amount_band_top_offset,
        page_max_x,
        anchor.bbox.bottom + tuning.amount_band_bottom_offset,
    );

    // 3. Join region text
    let text: String = region_words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    // 4. Extract 2-decimal amounts, exclude > max_valid_amount (tax IDs), take max
    //   价税合计 = 含税总额, always >= tax-exclusive amount, so max is correct
    let re = regex::Regex::new(r"([\d,]+\.\d{2})").ok()?;
    let mut max_amount: f64 = 0.0;
    for caps in re.captures_iter(&text) {
        let v: f64 = caps[1].replace(",", "").parse().unwrap_or(0.0);
        if v > max_amount && v < tuning.max_valid_amount {
            max_amount = v;
        }
    }

    if max_amount > 0.0 {
        Some(max_amount)
    } else {
        None
    }
}

// ── Word-to-OcrTextItem Conversion ─────────────────────────────────────

/// Convert raw pdfplumber `Word` values into `OcrTextItem` entries with
/// coordinate metadata.
pub fn words_to_items(words: &[Word]) -> Vec<OcrTextItem> {
    words
        .iter()
        .map(|w| OcrTextItem {
            text: w.text.clone(),
            confidence: 1.0,
            box_coords: Some(bbox_to_json(
                w.bbox.x0,
                w.bbox.top,
                w.bbox.x1,
                w.bbox.bottom,
                1.0,
            )),
        })
        .collect()
}

// ── Column-aware Line Merging ──────────────────────────────────────────

/// Merge words within a single column into lines by Y-coordinate proximity,
/// splitting items when the X-gap between adjacent words exceeds `x_gap_threshold`.
///
/// This is a column-safe alternative to `merge_words_into_lines` — it does not
/// merge across column boundaries because the caller pre-filters words by column.
///
/// Returns `Vec<(joined_text, merged_bbox)>`.
pub fn merge_words_in_column(
    words: &[&Word],
    y_tolerance: f64,
    x_gap_threshold: f64,
) -> Vec<(String, BBox)> {
    if words.is_empty() {
        return vec![];
    }

    // Sort by Y then by X
    let mut sorted: Vec<&Word> = words.to_vec();
    sorted.sort_by(|a, b| {
        a.bbox
            .top
            .partial_cmp(&b.bbox.top)
            .unwrap()
            .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
    });

    // Group by Y proximity
    let mut lines: Vec<Vec<&Word>> = Vec::new();
    for &word in &sorted {
        if let Some(last_line) = lines.last() {
            if (word.bbox.top - last_line[0].bbox.top).abs() > y_tolerance {
                lines.push(vec![word]);
            } else {
                lines.last_mut().unwrap().push(word);
            }
        } else {
            lines.push(vec![word]);
        }
    }

    // Within each line, split by X gap
    lines
        .into_iter()
        .flat_map(|line_words| {
            if line_words.is_empty() {
                return Vec::new();
            }
            let mut groups: Vec<Vec<&Word>> = vec![vec![line_words[0]]];
            for &w in &line_words[1..] {
                let prev = groups.last().unwrap().last().unwrap();
                if w.bbox.x0 - prev.bbox.x1 > x_gap_threshold {
                    groups.push(vec![w]);
                } else {
                    groups.last_mut().unwrap().push(w);
                }
            }

            groups
                .into_iter()
                .map(|group| {
                    let text = group
                        .iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let x0 = group
                        .iter()
                        .map(|w| w.bbox.x0)
                        .fold(f64::INFINITY, f64::min);
                    let top = group
                        .iter()
                        .map(|w| w.bbox.top)
                        .fold(f64::INFINITY, f64::min);
                    let x1 = group
                        .iter()
                        .map(|w| w.bbox.x1)
                        .fold(f64::NEG_INFINITY, f64::max);
                    let bottom = group
                        .iter()
                        .map(|w| w.bbox.bottom)
                        .fold(f64::NEG_INFINITY, f64::max);
                    (text, BBox::new(x0, top, x1, bottom))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber::{TextDirection, Word};

    // ── Helper ────────────────────────────────────────────────────────

    fn make_word(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Word {
        Word {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            doctop: top,
            direction: TextDirection::Ltr,
            chars: vec![],
            fontname: String::new(),
            size: 0.0,
            non_stroking_color: None,
            render_mode: 0,
            text_object_index: 0,
            font_flags: None,
            stem_v: None,
        }
    }

    // ── detect_columns ────────────────────────────────────────────────

    #[test]
    fn test_detect_columns_double_column() {
        // Simulates 滴滴电子发票A 双栏 layout:
        //   Left (买方):  X ~31-175
        //   Right (销售方): X ~301-450
        //   Wide gap between 175 and 301 (126px)
        let words = vec![
            // Left column (买方)
            make_word("名称", 31.0, 95.0, 60.0, 104.0),
            make_word("中国人民解放军国防科技大学", 60.0, 95.0, 175.0, 104.0),
            // Right column (销售方)
            make_word("销", 301.0, 91.0, 310.0, 100.0),
            make_word("名称", 315.0, 95.0, 345.0, 104.0),
            make_word("成都滴滴优行科技有限公司", 345.0, 95.0, 450.0, 104.0),
        ];

        let layout = detect_columns(&words);
        assert!(
            !layout.is_single_column(),
            "expected double-column layout"
        );
        let left = layout.left_column().expect("left column");
        let right = layout.right_column().expect("right column");
        assert!(
            right.x_min >= left.x_max,
            "right column should start at or after left column ends (right.x_min={}, left.x_max={})",
            right.x_min,
            left.x_max
        );
    }

    #[test]
    fn test_detect_columns_single_column() {
        // Simulates 滴滴行程单 — words uniformly distributed across the full X
        // range ~14-140. Words placed at alternating bins across two Y levels
        // so every histogram bin is filled (no empty run ≥ 3).
        // With bucket_width ~8.0, range 126 → 16 bins; words at bins 0,1,2,...
        let words = vec![
            // Y=100: fills even bins (0, 2, 4, ...)
            make_word("W1", 14.0, 100.0, 22.0, 112.0),
            make_word("W3", 30.0, 100.0, 38.0, 112.0),
            make_word("W5", 46.0, 100.0, 54.0, 112.0),
            make_word("W7", 62.0, 100.0, 70.0, 112.0),
            make_word("W9", 78.0, 100.0, 86.0, 112.0),
            make_word("W11", 94.0, 100.0, 102.0, 112.0),
            make_word("W13", 110.0, 100.0, 118.0, 112.0),
            make_word("W15", 126.0, 100.0, 134.0, 112.0),
            // Y=150: fills odd bins (1, 3, 5, ...)
            make_word("W2", 22.0, 150.0, 30.0, 162.0),
            make_word("W4", 38.0, 150.0, 46.0, 162.0),
            make_word("W6", 54.0, 150.0, 62.0, 162.0),
            make_word("W8", 70.0, 150.0, 78.0, 162.0),
            make_word("W10", 86.0, 150.0, 94.0, 162.0),
            make_word("W12", 102.0, 150.0, 110.0, 162.0),
            make_word("W14", 118.0, 150.0, 126.0, 162.0),
            make_word("W16", 134.0, 150.0, 142.0, 162.0),
        ];

        let layout = detect_columns(&words);
        assert!(layout.is_single_column(), "expected single-column layout");
    }

    #[test]
    fn test_detect_columns_empty() {
        let words = vec![];
        let layout = detect_columns(&words);
        assert!(layout.columns.is_empty());
    }

    #[test]
    fn test_detect_columns_only_header() {
        // All words have Y < 80 → treated as header, no body → empty columns
        let words = vec![make_word("Header", 10.0, 30.0, 60.0, 42.0)];
        let layout = detect_columns(&words);
        assert!(layout.columns.is_empty());
    }

    // ── find_anchor_word ──────────────────────────────────────────────

    #[test]
    fn test_find_anchor_word_found() {
        let words = vec![
            make_word("价税合计", 36.0, 293.0, 100.0, 304.0),
            make_word("小写", 36.0, 310.0, 70.0, 321.0),
        ];
        let found = find_anchor_word(&words, &["价税合计"]).expect("should find anchor");
        assert_eq!(found.text, "价税合计");
    }

    #[test]
    fn test_find_anchor_word_not_found() {
        let words = vec![make_word("hello", 10.0, 10.0, 50.0, 22.0)];
        assert!(find_anchor_word(&words, &["world"]).is_none());
    }

    #[test]
    fn test_find_anchor_word_multiple_keywords() {
        let words = vec![
            make_word("发票", 10.0, 10.0, 40.0, 22.0),
            make_word("名称：测试", 50.0, 10.0, 100.0, 22.0),
        ];
        // Should find the first word matching any keyword
        let found = find_anchor_word(&words, &["名称", "发票"]).expect("should find anchor");
        assert_eq!(found.text, "发票");
    }

    // ── extract_region_words ──────────────────────────────────────────

    #[test]
    fn test_extract_region_words_includes_center() {
        let words = vec![
            make_word("价税合计", 36.0, 293.0, 100.0, 304.0),
            make_word("523.57", 100.0, 294.0, 115.0, 303.0),
        ];
        // Bounding box around total area (0,283,600,305)
        let region = extract_region_words(&words, 0.0, 283.0, 600.0, 305.0);
        assert_eq!(region.len(), 2, "both words should be in the region");
        assert!(region.iter().any(|w| w.text == "价税合计"));
        assert!(region.iter().any(|w| w.text == "523.57"));
    }

    #[test]
    fn test_extract_region_words_excludes_outside() {
        let words = vec![
            make_word("价税合计", 36.0, 293.0, 100.0, 304.0),
            // Far above the region (Y=95 vs Y_min=283)
            make_word("名称", 315.0, 95.0, 345.0, 104.0),
        ];
        let region = extract_region_words(&words, 0.0, 283.0, 600.0, 305.0);
        assert_eq!(region.len(), 1);
        assert_eq!(region[0].text, "价税合计");
    }

    #[test]
    fn test_extract_region_words_empty() {
        let words = vec![make_word("a", 0.0, 0.0, 10.0, 10.0)];
        let region = extract_region_words(&words, 100.0, 100.0, 200.0, 200.0);
        assert!(region.is_empty());
    }

    // ── extract_seller_by_raw_coords ──────────────────────────────────

    #[test]
    fn test_extract_seller_by_raw_coords_rightmost() {
        // 买方 on left (X=31), 销售方 on right (X=315) — should pick rightmost
        let words = vec![
            make_word(
                "名称：中国人民解放军国防科技大学",
                31.0,
                95.0,
                175.0,
                104.0,
            ),
            make_word(
                "名称：成都滴滴优行科技有限公司",
                315.0,
                95.0,
                450.0,
                104.0,
            ),
        ];
        let seller = extract_seller_by_raw_coords(&words);
        assert_eq!(seller, "成都滴滴优行科技有限公司");
    }

    #[test]
    fn test_extract_seller_by_raw_coords_cleanup() {
        // Rightmost word has "销售方" artifact that should be cleaned
        let words = vec![
            make_word("名称：买_北京公司", 31.0, 95.0, 175.0, 104.0),
            make_word("名称：销售方上海公司", 315.0, 95.0, 450.0, 104.0),
        ];
        let seller = extract_seller_by_raw_coords(&words);
        assert_eq!(seller, "上海公司", "should strip '销售方' artifact");
    }

    #[test]
    fn test_extract_seller_by_raw_coords_no_match() {
        let words = vec![make_word("随便的文字", 10.0, 10.0, 50.0, 22.0)];
        assert_eq!(extract_seller_by_raw_coords(&words), "");
    }

    #[test]
    fn test_extract_seller_by_raw_coords_empty_words() {
        let words = vec![];
        assert_eq!(extract_seller_by_raw_coords(&words), "");
    }

    #[test]
    fn test_extract_seller_by_raw_coords_wide_seller_word() {
        // 飞猪发票场景：pdfplumber 产生宽 Word，卖方 Word 的 X0 反而比买方小，
        // 但卖方在右栏，X 中心点更大。用 X0 会误选取买方，用中心点正确选取卖方。
        let words = vec![
            // 买方：X0=34, X1=178, center=106
            make_word("名称：中国人民解放军国防科技大学", 34.0, 103.0, 178.0, 112.0),
            // 卖方：X0=20, X1=481, center=250（宽 Word 跨两栏，左边缘比买方还左）
            make_word("名称：阿斯兰航空服务（上海）有限公司销", 20.0, 103.0, 481.0, 112.0),
        ];
        let seller = extract_seller_by_raw_coords(&words);
        assert_eq!(seller, "阿斯兰航空服务（上海）有限公司");
    }

    #[test]
    fn test_extract_seller_by_raw_coords_seller_without_prefix() {
        // 天府通发票场景：只有买方带"名称："前缀，卖方是无前缀的公司名 word，
        // 在买方右侧同 Y 行。应识别卖方为右侧公司名。
        let words = vec![
            // 买方：带"名称："前缀，含买方关键词"国防"/"大学"
            make_word("名称：中国人民解放军国防科技大学", 57.0, 97.0, 344.0, 106.0),
            // 卖方：无"名称："前缀，纯公司名，在买方右侧同 Y 行
            make_word("成都天府通金融支付股份有限公司", 340.0, 97.0, 475.0, 106.0),
        ];
        let seller = extract_seller_by_raw_coords(&words);
        assert_eq!(seller, "成都天府通金融支付股份有限公司");
    }

    #[test]
    fn test_extract_seller_by_raw_coords_reversed_format() {
        // 电子发票（普通发票）场景：pdfplumber 将卖方名称与尾部标签合并为一个宽 Word，
        // 产生 "[公司名]名称：[标签]" 格式。应从 "名称：" 之前提取公司名。
        let words = vec![
            // 买方（宽 Word，含水印"载"前缀）
            make_word("载中国人民解放军国防科技大学系统工程学院", 60.0, 99.0, 596.0, 111.0),
            // 卖方：公司名 + "名称：" + 标签 "买"
            make_word("四川景澜酒店管理有限公司名称：名称：买", 20.0, 102.0, 454.0, 115.0),
        ];
        let seller = extract_seller_by_raw_coords(&words);
        assert_eq!(seller, "四川景澜酒店管理有限公司");
    }

    #[test]
    fn test_extract_company_name_before_label_basic() {
        // 标准场景：公司名在 "名称：" 之前
        let name = extract_company_name_before_label("四川景澜酒店管理有限公司");
        assert_eq!(name, "四川景澜酒店管理有限公司");
    }

    #[test]
    fn test_extract_company_name_before_label_with_label_chars() {
        // 带标签字符前缀：应过滤掉 "销" 等标签字符
        let name = extract_company_name_before_label("销成都铂涛酒店管理有限公司");
        assert_eq!(name, "成都铂涛酒店管理有限公司");
    }

    #[test]
    fn test_extract_company_name_before_label_no_suffix() {
        // 无公司后缀：应返回空字符串
        let name = extract_company_name_before_label("随便的文字");
        assert_eq!(name, "");
    }

    #[test]
    fn test_extract_company_name_before_label_empty() {
        let name = extract_company_name_before_label("");
        assert_eq!(name, "");
    }

    // ── words_to_items ────────────────────────────────────────────────

    #[test]
    fn test_words_to_items_basic() {
        let words = vec![make_word("测试文本", 10.0, 20.0, 100.0, 32.0)];
        let items = words_to_items(&words);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "测试文本");
        assert!((items[0].confidence - 1.0).abs() < 1e-6);
        assert!(items[0].box_coords.is_some(), "should have coordinates");
    }

    #[test]
    fn test_words_to_items_empty() {
        let words: Vec<Word> = vec![];
        let items = words_to_items(&words);
        assert!(items.is_empty());
    }

    #[test]
    fn test_words_to_items_multiple() {
        let words = vec![
            make_word("a", 0.0, 0.0, 10.0, 12.0),
            make_word("b", 20.0, 0.0, 30.0, 12.0),
        ];
        let items = words_to_items(&words);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "a");
        assert_eq!(items[1].text, "b");
    }

    // ── extract_amount_by_coords ──────────────────────────────────────

    #[test]
    fn test_extract_amount_by_coords_basic() {
        // 模拟滴滴A：价税合计锚点 + 金额同 Y 行
        let words = vec![
            make_word("¥价税合计（大写）", 36.0, 293.0, 445.0, 304.0),
            make_word("（小写）伍佰贰拾叁圆伍角柒分", 161.0, 293.0, 437.0, 302.0),
            make_word("523.57", 445.0, 294.0, 472.0, 303.0),
        ];
        let amt = extract_amount_by_coords(&words);
        assert!(amt.is_some());
        assert!((amt.unwrap() - 523.57).abs() < 0.001);
    }

    #[test]
    fn test_extract_amount_by_coords_excludes_items_table() {
        // items 表格的不含税金额在更上方 Y（应被排除），价税合计在下方
        let words = vec![
            make_word("584.73", 204.0, 163.0, 231.0, 172.0), // items 行，Y=163
            make_word(
                "价税合计（大写）",
                36.0,
                293.0,
                200.0,
                304.0,
            ), // total 锚点，Y=293
            make_word("523.57", 445.0, 294.0, 472.0, 303.0), // 含税金额
        ];
        let amt = extract_amount_by_coords(&words).expect("should find amount");
        assert!(
            (amt - 523.57).abs() < 0.001,
            "should pick 523.57 not 584.73"
        );
    }

    #[test]
    fn test_extract_amount_by_coords_no_anchor() {
        let words = vec![make_word("hello", 10.0, 10.0, 50.0, 22.0)];
        assert!(extract_amount_by_coords(&words).is_none());
    }

    #[test]
    fn test_extract_amount_by_coords_excludes_tax_id() {
        // 税号 > 1e6 应被排除
        let words = vec![
            make_word("价税合计", 36.0, 293.0, 100.0, 304.0),
            make_word(
                "91430100578607044B",
                200.0,
                295.0,
                400.0,
                304.0,
            ), // 税号
            make_word("6.30", 445.0, 294.0, 472.0, 303.0),
        ];
        let amt = extract_amount_by_coords(&words).expect("should find amount");
        assert!(
            (amt - 6.30).abs() < 0.001,
            "should pick 6.30 not tax id"
        );
    }

    // ── merge_words_in_column ─────────────────────────────────────────

    #[test]
    fn test_merge_words_in_column_same_line() {
        let w1 = make_word("Hello", 10.0, 100.0, 50.0, 112.0);
        let w2 = make_word("World", 55.0, 100.0, 95.0, 112.0);
        let refs = vec![&w1, &w2];
        let merged = merge_words_in_column(&refs, 6.0, 30.0);
        assert_eq!(merged.len(), 1, "should merge into one");
        assert_eq!(merged[0].0, "Hello World");
    }

    #[test]
    fn test_merge_words_in_column_split_by_gap() {
        let w1 = make_word("Left", 10.0, 100.0, 50.0, 112.0);
        let w2 = make_word("Right", 200.0, 100.0, 250.0, 112.0);
        let refs = vec![&w1, &w2];
        let merged = merge_words_in_column(&refs, 6.0, 30.0);
        assert_eq!(merged.len(), 2, "large gap should split");
        assert_eq!(merged[0].0, "Left");
        assert_eq!(merged[1].0, "Right");
    }

    #[test]
    fn test_merge_words_in_column_separate_lines() {
        let w1 = make_word("Line1", 10.0, 100.0, 50.0, 112.0);
        let w2 = make_word("Line2", 10.0, 200.0, 50.0, 212.0);
        let refs = vec![&w1, &w2];
        let merged = merge_words_in_column(&refs, 6.0, 30.0);
        assert_eq!(merged.len(), 2, "different Y → separate lines");
        assert_eq!(merged[0].0, "Line1");
        assert_eq!(merged[1].0, "Line2");
    }

    #[test]
    fn test_merge_words_in_column_empty() {
        let refs: Vec<&Word> = vec![];
        let merged = merge_words_in_column(&refs, 6.0, 30.0);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_merge_words_in_column_bbox_union() {
        let w1 = make_word("A", 10.0, 100.0, 30.0, 112.0);
        let w2 = make_word("B", 35.0, 100.0, 55.0, 112.0);
        let refs = vec![&w1, &w2];
        let merged = merge_words_in_column(&refs, 6.0, 30.0);
        assert_eq!(merged.len(), 1);
        let (_text, bbox) = &merged[0];
        assert!((bbox.x0 - 10.0).abs() < 1e-6);
        assert!((bbox.top - 100.0).abs() < 1e-6);
        assert!((bbox.x1 - 55.0).abs() < 1e-6);
        assert!((bbox.bottom - 112.0).abs() < 1e-6);
    }
}
