//! Layout extractor — column detection, region extraction, and seller coordinate
//! extraction operating directly on raw pdfplumber `Word` coordinates (before
//! line-merging), preserving multi-column invoice layout.
//!
//! All functionality is gated behind `#[cfg(feature = "pdfplumber")]`.

use pdfplumber::{BBox, Word};

use crate::ocr::engine::bbox_to_json;
use crate::ocr::OcrTextItem;

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
/// 1. Exclude header words (`top < 80`).
/// 2. Compute average word height → derive bucket width (`max(avg_height × 0.6, 8.0)`).
/// 3. Bin body words by X, find runs of ≥3 consecutive empty bins as column gaps.
/// 4. Take the widest gap, split at its center → left/right columns.
/// 5. No gap → single `Full` column.
pub fn detect_columns(words: &[Word]) -> ColumnarLayout {
    if words.is_empty() {
        return ColumnarLayout { columns: vec![] };
    }

    // Exclude header words (Y < 80)
    let body_words: Vec<&Word> = words.iter().filter(|w| w.bbox.top >= 80.0).collect();
    if body_words.is_empty() {
        return ColumnarLayout { columns: vec![] };
    }

    // Compute average word height for adaptive bucket sizing
    let sum_h: f64 = body_words.iter().map(|w| w.bbox.height()).sum();
    let avg_height = sum_h / body_words.len() as f64;
    let bucket_width = (avg_height * 0.6).max(8.0);

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
    let gap_threshold = 3;
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
        clean_seller_name(&text[pos + "名称：".len()..])
    } else if let Some(pos) = text.find("名称:") {
        clean_seller_name(&text[pos + "名称:".len()..])
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
/// Used to detect when the rightmost "名称：" candidate is actually the buyer,
/// triggering a search for the real seller to its right.
fn is_likely_buyer(name: &str) -> bool {
    const BUYER_KEYWORDS: &[&str] = &["国防", "大学", "学院", "医院", "购买方"];
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
/// 5. Return the maximum amount (< 1,000,000 to exclude tax IDs).
///
/// Returns `None` if no anchor is found or no valid amount exists in the region.
pub fn extract_amount_by_coords(words: &[Word]) -> Option<f64> {
    // 1. Find anchor word
    let anchor = find_anchor_word(words, &["价税合计", "合计金额", "总金额"])?;

    // 2. Define compact Y-band
    let page_max_x = words
        .iter()
        .map(|w| w.bbox.x1)
        .fold(0.0f64, f64::max);
    let region_words = extract_region_words(
        words,
        0.0,
        anchor.bbox.top - 5.0,
        page_max_x,
        anchor.bbox.bottom + 30.0,
    );

    // 3. Join region text
    let text: String = region_words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    // 4. Extract 2-decimal amounts, exclude > 1e6 (tax IDs), take max
    //   价税合计 = 含税总额, always >= tax-exclusive amount, so max is correct
    let re = regex::Regex::new(r"([\d,]+\.\d{2})").ok()?;
    let mut max_amount: f64 = 0.0;
    for caps in re.captures_iter(&text) {
        let v: f64 = caps[1].replace(",", "").parse().unwrap_or(0.0);
        if v > max_amount && v < 1_000_000.0 {
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
