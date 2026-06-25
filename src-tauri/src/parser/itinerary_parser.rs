use std::collections::HashMap;

use crate::models::invoice::Itinerary;
use crate::ocr::OcrTextItem;
use regex::Regex;

pub fn parse_itinerary_text(texts: &[OcrTextItem]) -> Vec<Itinerary> {
    let all_text: String = texts
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let mut itineraries = parse_itinerary_text_impl(&all_text);
    if !itineraries.is_empty() {
        enrich_itinerary_years(&mut itineraries, &all_text);
    }
    itineraries
}

fn parse_itinerary_text_impl(all_text: &str) -> Vec<Itinerary> {
    let mut itineraries = Vec::new();

    // 格式1：OCR 输出，带 ¥ 符号  2025-08-05 09:30  滴滴出行  ¥35.00
    let re = Regex::new(
        r"(?m)(\d{4}[-/]\d{2}[-/]\d{2}\s+\d{2}:\d{2})\s+(.+?)\s+[¥￥]\s*([\d.]+)",
    )
    .unwrap();

    for cap in re.captures_iter(all_text) {
        itineraries.push(Itinerary {
            date_time: cap[1].to_string(),
            provider: cap[2].trim().to_string(),
            pickup: String::new(),
            dropoff: String::new(),
            amount: cap[3].parse().unwrap_or(0.0),
        });
    }

    if !itineraries.is_empty() {
        return itineraries;
    }

    // 格式2：parangi 提取的表格格式
    // 匹配：序号 车型 MM-DD HH: 城市 ... 里程 金额
    // 例：1 专车 04-22 21: 成都 ... 60.6 195.37
    let re_table = Regex::new(
        r"(\d+)\s+\S+\s+(\d{2}-\d{2}\s+\d{2}:\d{0,2})\s+\S+\s+"
    ).unwrap();

    let lines: Vec<&str> = all_text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(cap) = re_table.captures(line) {
            let _seq: u32 = cap[1].parse().unwrap_or(0);
            let date_time = cap[2].trim().to_string();
            let nums = extract_trailing_numbers(line);
            if nums.len() >= 2 {
                let amount = nums[nums.len() - 1];
                if amount > 0.0 {
                    itineraries.push(Itinerary {
                        date_time,
                        provider: String::new(),
                        pickup: String::new(),
                        dropoff: String::new(),
                        amount,
                    });
                }
            }
        }
        i += 1;
    }

    if !itineraries.is_empty() {
        return itineraries;
    }

    // 格式3：天府通格式（按行逐条匹配）
    if all_text.contains("天府通")
        || all_text.contains("电子行程单")
            && (all_text.contains("公交") || all_text.contains("地铁"))
    {
        let tft_entries = parse_tianfutong_format(all_text);
        if !tft_entries.is_empty() {
            return tft_entries;
        }
    }

    if !itineraries.is_empty() {
        return itineraries;
    }

    // 格式4：回退，找 ¥ 金额
    parse_fallback_format(all_text)
}

/// 利用 OCR 坐标信息解析行程单表格（通用，不限于天府通）
///
/// 思路：
/// 1. 动态从表头提取所有列名及其 X 坐标
/// 2. 语义映射：列名关键词 → 逻辑字段（time/pickup/dropoff/amount/provider）
/// 3. 用"时间"列的块做锚点确定每条行程的 Y 范围
/// 4. 在 Y 范围内聚合各列文本，提取字段值
///
/// 支持的天府通/高德/滴滴等不同列名：
///   天府通：出行时间 / 进出站 / 金额
///   高德：  上车时间 / 起点 / 终点 / 金额 / 服务商
///   滴滴：  上车时间 / 起点 / 终点 / 金额 / 车型
pub fn parse_itinerary_with_coords_pages(pages: &[crate::ocr::OcrPageResult]) -> Vec<Itinerary> {
    parse_itinerary_with_coords_pages_and_fallback(pages, None)
}

pub fn parse_itinerary_with_coords_pages_and_fallback(
    pages: &[crate::ocr::OcrPageResult],
    fallback_texts: Option<&[OcrTextItem]>,
) -> Vec<Itinerary> {
    let mut all = Vec::new();
    for page in pages {
        let result = parse_itinerary_with_coords(&page.texts);
        all.extend(result);
    }
    if !all.is_empty() {
        if let Some(fb) = fallback_texts {
            cross_validate_amounts(&mut all, fb);
            // 用 fallback 文本（含行程单顶部时间区间）补全无年份的行程 date_time
            let fb_text: String = fb.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join("\n");
            enrich_itinerary_years(&mut all, &fb_text);
        }
        return all;
    }
    if let Some(fb) = fallback_texts {
        let result = parse_itinerary_text(fb);
        if !result.is_empty() {
            return result;
        }
    }
    let all_texts: Vec<OcrTextItem> = pages.iter().flat_map(|p| p.texts.clone()).collect();
    parse_itinerary_text(&all_texts)
}

pub fn parse_itinerary_with_coords(texts: &[OcrTextItem]) -> Vec<Itinerary> {
    let positioned: Vec<PositionedText> = texts
        .iter()
        .filter_map(|item| {
            let (x, y) = extract_coords(&item.box_coords)?;
            Some(PositionedText {
                text: item.text.trim().to_string(),
                x,
                y,
            })
        })
        .filter(|p| !p.text.is_empty())
        .collect();

    if positioned.len() < 4 {
        return parse_itinerary_text(texts);
    }

    parse_table_generic(&positioned).unwrap_or_else(|| parse_itinerary_text(texts))
}

struct PositionedText {
    text: String,
    x: f64,
    y: f64,
}

/// 从 box_coords 中提取中心坐标
fn extract_coords(box_coords: &Option<serde_json::Value>) -> Option<(f64, f64)> {
    let points = box_coords.as_ref()?.get("points")?.as_array()?;
    if points.len() < 4 {
        return None;
    }
    let xs: Vec<f64> = points.iter().filter_map(|p| p.get("x")?.as_f64()).collect();
    let ys: Vec<f64> = points.iter().filter_map(|p| p.get("y")?.as_f64()).collect();
    if xs.is_empty() || ys.is_empty() {
        return None;
    }
    let cx = (xs.iter().cloned().fold(f64::INFINITY, f64::min)
        + xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
        / 2.0;
    let cy = (ys.iter().cloned().fold(f64::INFINITY, f64::min)
        + ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
        / 2.0;
    Some((cx, cy))
}

/// 语义列映射：关键词 → 逻辑字段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SemanticCol {
    Seq,
    Time,
    Pickup,
    Dropoff,
    Amount,
    Provider,
}

const COL_KEYWORDS: &[(SemanticCol, &[&str])] = &[
    (SemanticCol::Seq, &["序号"]),
    (SemanticCol::Time, &["出行时间", "上车时间", "时间"]),
    (SemanticCol::Pickup, &["起点", "进站"]),
    (SemanticCol::Dropoff, &["终点", "出站"]),
    (SemanticCol::Amount, &["金额", "元"]),
    (SemanticCol::Provider, &["服务商", "行程类型", "车型"]),
];

/// 通用表格行程单解析
///
/// 1. 从表头行动态提取所有列名和 X 坐标
/// 2. 语义映射到逻辑字段
/// 3. 用"时间"列锚定行程 Y 范围
/// 4. 聚合 Y 范围内各列文本提取字段
fn detect_merged_distance_amount(header: &[&PositionedText]) -> bool {
    header.iter().any(|p| {
        p.text.contains("金额") && (p.text.contains("里程") || p.text.contains("公里"))
    })
}

fn parse_table_generic(positioned: &[PositionedText]) -> Option<Vec<Itinerary>> {
    let header = find_header(positioned)?;

    let col_map: Vec<(SemanticCol, f64)> = COL_KEYWORDS
        .iter()
        .filter_map(|(sem, kws)| {
            let x = find_col_x(&header, kws)?;
            Some((*sem, x))
        })
        .collect();

    let has_amount = col_map.iter().any(|(s, _)| *s == SemanticCol::Amount);
    if !has_amount {
        return None;
    }

    let merged_dist_amt = detect_merged_distance_amount(&header);

    let header_bottom = header.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let data: Vec<&PositionedText> = positioned
        .iter()
        .filter(|p| p.y > header_bottom + 5.0)
        .collect();

    if data.is_empty() {
        return None;
    }

    let col_span = estimate_col_span_from_header(&header).unwrap_or_else(|| estimate_col_span(&data));
    let seq_x = col_map.iter().find(|(s, _)| *s == SemanticCol::Seq).map(|(_, x)| *x);
    let avg_row_h = estimate_row_height(&data, seq_x.unwrap_or(0.0), col_span);

    let time_x = col_map.iter().find(|(s, _)| *s == SemanticCol::Time).map(|(_, x)| *x);

    let re_datetime_full = Regex::new(r"(\d{4}-\d{2}-\d{2})\s*(\d{2}:\d{2}:\d{2})").unwrap();
    let re_datetime_full_nosec = Regex::new(r"(\d{4}-\d{2}-\d{2})\s*(\d{1,2}:\d{2})").unwrap();
    let re_datetime_full_nospace = Regex::new(r"(\d{4}-\d{2}-\d{2})(\d{1,2}:\d{2})").unwrap();
    let re_datetime_short = Regex::new(r"(\d{2}-\d{2})\s*(\d{1,2}:\d{2})").unwrap();
    let re_datetime_merged = Regex::new(r"(\d{2}-\d{2})(\d{1,2})[:：]").unwrap();
    let re_datetime_short_garbled = Regex::new(r"(\d{2}-\d{2})(\d{1,2})").unwrap();
    let re_amount = Regex::new(r"(\d+(?:\.\d+)?)").unwrap();

    let pickup_x = col_map.iter().find(|(s, _)| *s == SemanticCol::Pickup).map(|(_, x)| *x);
    let dropoff_x = col_map.iter().find(|(s, _)| *s == SemanticCol::Dropoff).map(|(_, x)| *x);
    let amount_x = col_map.iter().find(|(s, _)| *s == SemanticCol::Amount)?.1;
    let provider_x = col_map.iter().find(|(s, _)| *s == SemanticCol::Provider).map(|(_, x)| *x);

    let col_boundaries = build_col_boundaries(&header, &col_map);

    // Build anchors: combine seq + time
    // 1. Collect seq anchors
    let mut anchor_ys: Vec<f64> = Vec::new();
    if let Some(sx) = seq_x {
        let mut seq_anchors: Vec<&PositionedText> = data
            .iter()
            .filter(|p| (p.x - sx).abs() <= col_span * 0.5 && extract_seq_number(&p.text).is_some())
            .copied()
            .collect();
        seq_anchors.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
        let mut seen = std::collections::HashSet::new();
        for p in &seq_anchors {
            let n = extract_seq_number(&p.text).unwrap_or(0);
            if seen.insert(n) {
                anchor_ys.push(p.y);
            }
        }
    }

    // 2. Add time anchors not near any existing anchor
    if let Some(tx) = time_x {
        let time_blocks: Vec<&&PositionedText> = data
            .iter()
            .filter(|p| {
                (p.x - tx).abs() <= col_span * 0.5 && looks_like_datetime(&p.text)
            })
            .collect();
        for tb in &time_blocks {
            let near_existing = anchor_ys.iter().any(|&ay| (ay - tb.y).abs() < avg_row_h * 0.5);
            if !near_existing {
                anchor_ys.push(tb.y);
            }
        }
    }

    anchor_ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // 3. Fill gaps: if gap > 1.8 * avg_row_h, look for amount blocks in the middle
    if anchor_ys.len() >= 2 {
        let mut gap_anchors: Vec<f64> = Vec::new();
        for i in 0..anchor_ys.len() - 1 {
            let gap = anchor_ys[i + 1] - anchor_ys[i];
            if gap > avg_row_h * 1.8 {
                let gap_lo = anchor_ys[i] + avg_row_h * 0.5;
                let gap_hi = anchor_ys[i + 1] - avg_row_h * 0.5;
                let mut amount_ys: Vec<f64> = data
                    .iter()
                    .filter(|p| {
                        p.y > gap_lo && p.y < gap_hi && {
                            let in_amt_col = if let Some((xlo, xhi)) = col_boundaries.get(&SemanticCol::Amount) {
                                p.x >= *xlo && p.x <= *xhi
                            } else {
                                (p.x - amount_x).abs() <= col_span * 0.5
                            };
                            in_amt_col && re_amount.is_match(&p.text)
                        }
                    })
                    .map(|p| p.y)
                    .collect();
                amount_ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let mut last_y = 0.0f64;
                for &ay in &amount_ys {
                    if (ay - last_y).abs() > avg_row_h * 0.3 {
                        gap_anchors.push(ay);
                        last_y = ay;
                    }
                }
            }
        }
        anchor_ys.extend(gap_anchors);
        anchor_ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    }

    if anchor_ys.is_empty() {
        return None;
    }

    let mut entries = Vec::new();
    for (idx, &y_center) in anchor_ys.iter().enumerate() {
        let y_lo = y_center - avg_row_h * 0.5;
        let y_hi = if idx + 1 < anchor_ys.len() {
            let next_y = anchor_ys[idx + 1];
            (y_center + next_y) / 2.0
        } else {
            y_center + avg_row_h * 3.0
        };

        // Collect main-line blocks (Y near y_center) and continuation blocks (below)
        let main_y_tolerance = avg_row_h * 0.3;
        let main: Vec<&PositionedText> = data
            .iter()
            .filter(|p| (p.y - y_center).abs() <= main_y_tolerance)
            .copied()
            .collect();
        let cont: Vec<&PositionedText> = data
            .iter()
            .filter(|p| p.y > y_center + main_y_tolerance && p.y < y_hi)
            .copied()
            .collect();

        let date_time = if let Some(tx) = time_x {
            let raw_time_text = if let Some((xlo, xhi)) = col_boundaries.get(&SemanticCol::Time) {
                let main_text = filter_collect_by_x(&main, *xlo, *xhi);
                let cont_text = filter_collect_by_x(&cont, *xlo, *xhi);
                if cont_text.is_empty() { main_text } else { format!("{} {}", main_text, cont_text) }
            } else {
                collect_col_in_range(&data, tx, col_span, y_lo, y_hi)
            };
            let time_text = clean_time_text(&raw_time_text);
            if let Some(c) = re_datetime_full.captures(&time_text) {
                format!("{} {}", &c[1], &c[2])
            } else if let Some(c) = re_datetime_full_nospace.captures(&time_text) {
                format!("{} {}", &c[1], &c[2])
            } else if let Some(c) = re_datetime_full_nosec.captures(&time_text) {
                format!("{} {}", &c[1], &c[2])
            } else if let Some(c) = re_datetime_short.captures(&time_text) {
                format!("{} {}", &c[1], &c[2])
            } else if let Some(c) = re_datetime_merged.captures(&time_text) {
                format!("{} {}:??", &c[1], &c[2])
            } else if let Some(c) = re_datetime_short_garbled.captures(&time_text) {
                format!("{} {}:??", &c[1], &c[2])
            } else {
                time_text.trim().to_string()
            }
        } else {
            String::new()
        };

        let amount_text = if let Some((xlo, xhi)) = col_boundaries.get(&SemanticCol::Amount) {
            if merged_dist_amt {
                let split_x = amount_x;
                let text = collect_col_nearest(&data, split_x, *xhi, y_lo, y_hi, y_center);
                if text.is_empty() { collect_col_nearest(&data, *xlo, *xhi, y_lo, y_hi, y_center) } else { text }
            } else {
                let amount_x_val = (*xlo + *xhi) / 2.0;
                let text = collect_col_nearest(&data, amount_x_val, *xhi, y_lo, y_hi, y_center);
                if text.is_empty() { collect_col_nearest(&data, *xlo, *xhi, y_lo, y_hi, y_center) } else { text }
            }
        } else {
            let (xlo, xhi) = if merged_dist_amt {
                (amount_x, amount_x + col_span * 0.5)
            } else {
                (amount_x - col_span * 0.25, amount_x + col_span * 0.5)
            };
            collect_col_nearest(&data, xlo, xhi, y_lo, y_hi, y_center)
        };
        let amount: f64 = re_amount
            .captures(&amount_text)
            .and_then(|c| c[1].parse().ok())
            .unwrap_or(0.0);

        let (pickup, dropoff, provider) = if let (Some(_px), Some(_dx)) = (pickup_x, dropoff_x) {
            let pickup_text = collect_text_main_cont(
                &main, &cont, &col_boundaries, SemanticCol::Pickup, pickup_x, col_span,
            );
            let dropoff_text = collect_text_main_cont(
                &main, &cont, &col_boundaries, SemanticCol::Dropoff, dropoff_x, col_span,
            );

            let pickup = extract_pickup(&pickup_text);
            let dropoff = extract_dropoff(&dropoff_text);

            let provider = provider_x.map_or(String::new(), |_pvx| {
                let pv_main = filter_provider_blocks(&main, seq_x, time_x, col_span, &col_boundaries);
                let pv_cont = filter_provider_blocks(&cont, seq_x, time_x, col_span, &col_boundaries);
                let pv_text = if pv_cont.is_empty() { pv_main } else { format!("{} {}", pv_main, pv_cont) };
                let words: Vec<&str> = pv_text.split_whitespace().collect();
                words.into_iter().find(|w| !is_seq_number(w)).unwrap_or("").to_string()
            });

            (pickup, dropoff, provider)
        } else if let (Some(_px), None) | (None, Some(_px)) = (pickup_x, dropoff_x) {
            let station_x = pickup_x.or(dropoff_x).unwrap();
            let station_text = if let Some((xlo, xhi)) = col_boundaries.get(&SemanticCol::Pickup) {
                let main_text = filter_collect_by_x(&main, *xlo, *xhi);
                let cont_text = filter_collect_by_x(&cont, *xlo, *xhi);
                if cont_text.is_empty() { main_text } else { format!("{} {}", main_text, cont_text) }
            } else {
                collect_col_in_range(&data, station_x, col_span, y_lo, y_hi)
            };
            let re_pickup_tft = Regex::new(r"进站[：:]\s*([^\s~出]+)").unwrap();
            let re_dropoff_tft = Regex::new(r"出站[：:]\s*([^\s~进]+)|(?:^|[^进出])站[：:]\s*([^\s~进]+)").unwrap();
            let pickup = re_pickup_tft
                .captures(&station_text)
                .map(|c| c[1].to_string())
                .unwrap_or_default();
            let dropoff = re_dropoff_tft
                .captures(&station_text)
                .and_then(|c| {
                    c.get(1)
                        .or_else(|| c.get(2))
                        .map(|m| m.as_str().to_string())
                })
                .unwrap_or_default();
            (pickup, dropoff, "天府通".to_string())
        } else {
            (String::new(), String::new(), String::new())
        };

        if amount > 0.0 {
            entries.push(Itinerary {
                date_time,
                provider,
                pickup,
                dropoff,
                amount,
            });
        }
    }

    // Deduplicate by (date_time, amount)
    {
        let mut seen = std::collections::HashSet::new();
        entries.retain(|e| {
            let key = (e.date_time.clone(), (e.amount * 100.0).round() as i64);
            seen.insert(key)
        });
    }

    if entries.is_empty() { None } else { Some(entries) }
}

fn build_col_boundaries(header: &[&PositionedText], col_map: &[(SemanticCol, f64)]) -> HashMap<SemanticCol, (f64, f64)> {
    let mut all_xs: Vec<f64> = header.iter().map(|p| p.x).collect();
    all_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let mut boundaries = HashMap::new();
    for (sem, x) in col_map {
        let pos = all_xs.iter().position(|&hx| (hx - *x).abs() < 5.0).unwrap_or_else(|| {
            all_xs.iter().enumerate().min_by(|(_, a), (_, b)| (*a - x).abs().partial_cmp(&(*b - x).abs()).unwrap_or(std::cmp::Ordering::Equal)).unwrap().0
        });
        let left = if pos == 0 { 0.0 } else { (all_xs[pos - 1] + all_xs[pos]) / 2.0 };
        let right = if pos + 1 < all_xs.len() { (all_xs[pos] + all_xs[pos + 1]) / 2.0 } else { all_xs[pos] + 200.0 };
        boundaries.insert(*sem, (left, right));
    }
    boundaries
}

fn is_seq_number(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.contains('-') || trimmed.contains(':') || trimmed.contains('：') {
        return false;
    }
    if let Ok(n) = trimmed.parse::<u32>() {
        return n <= 200;
    }
    trimmed.split(|c: char| !c.is_ascii_digit()).next().map_or(false, |s| !s.is_empty() && s.parse::<u32>().map_or(false, |n| n <= 200))
}

fn extract_seq_number(text: &str) -> Option<u32> {
    let trimmed = text.trim();
    if trimmed.contains('-') || trimmed.contains(':') || trimmed.contains('：') {
        return None;
    }
    if let Ok(n) = trimmed.parse::<u32>() {
        if n <= 200 { return Some(n); }
        return None;
    }
    trimmed.split(|c: char| !c.is_ascii_digit()).next().and_then(|s| s.parse::<u32>().ok().filter(|&n| n <= 200))
}

fn looks_like_datetime(text: &str) -> bool {
    let re_full = Regex::new(r"^\d{4}-\d{2}-\d{2}").unwrap();
    let re_short_digit = Regex::new(r"^\d{2}-\d{2}\d").unwrap();
    let re_short_colon = Regex::new(r"^\d{2}-\d{2}\s*\d{0,2}[:：]").unwrap();
    re_full.is_match(text) || re_short_digit.is_match(text) || re_short_colon.is_match(text)
}

fn clean_time_text(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.iter().any(|w| is_seq_number(w)) {
        let filtered: Vec<&str> = words.into_iter().filter(|w| !is_seq_number(w)).collect();
        if filtered.is_empty() { text.to_string() } else { filtered.join(" ") }
    } else {
        text.to_string()
    }
}

fn collect_col_in_range(
    data: &[&PositionedText],
    col_x: f64,
    col_span: f64,
    y_lo: f64,
    y_hi: f64,
) -> String {
    collect_col_in_range_impl(data, col_x - col_span * 0.5, col_x + col_span * 0.5, y_lo, y_hi)
}

fn collect_col_nearest(
    data: &[&PositionedText],
    x_lo: f64,
    x_hi: f64,
    y_lo: f64,
    y_hi: f64,
    y_prefer: f64,
) -> String {
    let mut items: Vec<&PositionedText> = data
        .iter()
        .filter(|p| p.y >= y_lo && p.y < y_hi && p.x >= x_lo && p.x <= x_hi)
        .copied()
        .collect();
    items.sort_by(|a, b| {
        let da = (a.y - y_prefer).abs();
        let db = (b.y - y_prefer).abs();
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    if items.is_empty() {
        return String::new();
    }
    let best_y = items[0].y;
    let threshold = if items.len() > 1 {
        let ys: Vec<f64> = items.iter().map(|p| p.y).collect();
        let min_diff = ys.windows(2).map(|w| (w[1] - w[0]).abs()).filter(|d| *d > 1.0).fold(f64::INFINITY, f64::min);
        (min_diff * 0.4).max(15.0)
    } else {
        50.0
    };
    items
        .iter()
        .filter(|p| (p.y - best_y).abs() <= threshold)
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn filter_collect_by_x(items: &[&PositionedText], x_lo: f64, x_hi: f64) -> String {
    let mut filtered: Vec<&&PositionedText> = items
        .iter()
        .filter(|p| p.x >= x_lo && p.x <= x_hi)
        .collect();
    filtered.sort_by(|a, b| {
        a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });
    filtered
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn collect_text_main_cont(
    main: &[&PositionedText],
    cont: &[&PositionedText],
    col_boundaries: &HashMap<SemanticCol, (f64, f64)>,
    col: SemanticCol,
    col_x: Option<f64>,
    col_span: f64,
) -> String {
    let (main_text, cont_text) = if let Some((xlo, xhi)) = col_boundaries.get(&col) {
        (filter_collect_by_x(main, *xlo, *xhi), filter_collect_by_x(cont, *xlo, *xhi))
    } else if let Some(cx) = col_x {
        (
            collect_col_in_range_impl(main, cx - col_span * 0.5, cx + col_span * 0.5, f64::NEG_INFINITY, f64::INFINITY),
            collect_col_in_range_impl(cont, cx - col_span * 0.5, cx + col_span * 0.5, f64::NEG_INFINITY, f64::INFINITY),
        )
    } else {
        return String::new();
    };
    if cont_text.is_empty() {
        main_text
    } else {
        format!("{} {}", main_text, cont_text)
    }
}

fn filter_provider_blocks(
    items: &[&PositionedText],
    _seq_x: Option<f64>,
    _time_x: Option<f64>,
    _col_span: f64,
    col_boundaries: &HashMap<SemanticCol, (f64, f64)>,
) -> String {
    let (xlo, xhi) = if let Some((lo, hi)) = col_boundaries.get(&SemanticCol::Provider) {
        (*lo, *hi)
    } else {
        return String::new();
    };
    let mut filtered: Vec<&&PositionedText> = items
        .iter()
        .filter(|p| {
            if p.x < xlo || p.x > xhi { return false; }
            if looks_like_datetime(&p.text) { return false; }
            if p.text.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') { return false; }
            true
        })
        .collect();
    filtered.sort_by(|a, b| {
        a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });
    filtered
        .iter()
        .map(|p| {
            if is_seq_number(&p.text) {
                extract_provider_from_merged(&p.text)
            } else {
                p.text.as_str()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_provider_from_merged(text: &str) -> &str {
    let trimmed = text.trim();
    let digits_end = trimmed.char_indices()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let rest = &trimmed[digits_end..];
    if rest.is_empty() { "" } else { rest.trim() }
}

fn collect_col_in_range_impl(
    data: &[&PositionedText],
    x_lo: f64,
    x_hi: f64,
    y_lo: f64,
    y_hi: f64,
) -> String {
    let mut items: Vec<&PositionedText> = data
        .iter()
        .filter(|p| p.y >= y_lo && p.y < y_hi && p.x >= x_lo && p.x <= x_hi)
        .copied()
        .collect();
    items.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
    items
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 从行程单全文提取年份（顶部时间区间优先，回退全文第一个 20XX）。
/// 匹配 "2026年"、"2026-04-22"、"2026/04/22" 等格式中的年份。
fn extract_year_from_text(all_text: &str) -> Option<i32> {
    let re = Regex::new(r"20\d{2}").unwrap();
    re.captures(all_text)
        .and_then(|c| c[0].parse::<i32>().ok())
        .filter(|y| *y >= 2020 && *y <= 2100)
}

/// 用全文提取的年份补全无年份的行程 date_time。
/// "MM-DD ..." → "YYYY-MM-DD ..."；已有年份的不变。
fn enrich_itinerary_years(entries: &mut [Itinerary], all_text: &str) {
    let year = match extract_year_from_text(all_text) {
        Some(y) => y,
        None => return,
    };
    // 仅匹配以 "MM-DD" 开头（无年份）的 date_time。
    // "YYYY-MM-DD" 因第3字符非 '-' 不会被误匹配。
    let re_no_year = Regex::new(r"^(\d{2})-(\d{2})(.*)").unwrap();
    for entry in entries.iter_mut() {
        if let Some(cap) = re_no_year.captures(&entry.date_time) {
            entry.date_time = format!("{}-{}-{}{}", year, &cap[1], &cap[2], &cap[3]);
        }
    }
}

fn cross_validate_amounts(entries: &mut [Itinerary], fallback_texts: &[OcrTextItem]) {
    let all_text: String = fallback_texts
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let ref_amounts = extract_reference_amounts_ordered(&all_text);
    if !ref_amounts.is_empty() && ref_amounts.len() == entries.len() {
        for (i, entry) in entries.iter_mut().enumerate() {
            let ref_amt = ref_amounts[i];
            if ref_amt > 0.0 && (entry.amount - ref_amt).abs() > 0.005 {
                entry.amount = ref_amt;
            }
        }
    }

    let ref_providers = extract_reference_providers_ordered(&all_text);
    if !ref_providers.is_empty() && ref_providers.len() == entries.len() {
        for (i, entry) in entries.iter_mut().enumerate() {
            let ref_pv = &ref_providers[i];
            if !ref_pv.is_empty()
                && (entry.provider.is_empty()
                    || entry.provider.chars().all(|c| c.is_ascii_digit())
                    || entry.provider.chars().count() <= 1
                    || ref_pv.starts_with(&entry.provider.as_str()))
            {
                entry.provider = ref_pv.clone();
            }
        }
    }

    let ref_times = extract_reference_times_ordered(&all_text);
    if !ref_times.is_empty() && ref_times.len() == entries.len() {
        for (i, entry) in entries.iter_mut().enumerate() {
            if is_time_garbled(&entry.date_time) {
                entry.date_time = ref_times[i].clone();
            }
        }
    }
}

fn extract_reference_amounts_ordered(all_text: &str) -> Vec<f64> {
    let mut results = Vec::new();

    let re_didi = Regex::new(
        r"(?m)^\d+\s+\S+\s+\d{2}-\d{2}\s+\d{1,2}[:：].*?([\d.]+)\s*$"
    ).unwrap();
    for cap in re_didi.captures_iter(all_text) {
        if let Ok(amount) = cap[1].parse::<f64>() {
            results.push(amount);
        }
    }

    if !results.is_empty() {
        return results;
    }

    let re_gaode = Regex::new(
        r"(?m)(?:^|\n)\d+\s+\S+.*?([\d.]+)元"
    ).unwrap();
    for cap in re_gaode.captures_iter(all_text) {
        if let Ok(amount) = cap[1].parse::<f64>() {
            results.push(amount);
        }
    }

    results
}

fn extract_reference_providers_ordered(all_text: &str) -> Vec<String> {
    let re_didi_main = Regex::new(
        r"(?m)^(\d+)\s+(\S+)\s+\d{2}-\d{2}\s+\d{1,2}[:：]"
    ).unwrap();
    let re_didi_cont = Regex::new(
        r"^\s*(轻享|特快|甄选|快车)\s"
    ).unwrap();

    let main_matches: Vec<(u32, String)> = re_didi_main
        .captures_iter(all_text)
        .filter_map(|cap| {
            let seq: u32 = cap[1].parse().ok()?;
            let pv = cap[2].to_string();
            Some((seq, pv))
        })
        .collect();

    if !main_matches.is_empty() {
        let cont_suffixes: HashMap<u32, String> = {
            let mut map = HashMap::new();
            let lines: Vec<&str> = all_text.lines().collect();
            let mut last_seq: Option<u32> = None;
            for line in &lines {
                if let Some(cap) = re_didi_main.captures(line) {
                    if let Ok(seq) = cap[1].parse::<u32>() {
                        last_seq = Some(seq);
                    }
                    continue;
                }
                if let Some(cap) = re_didi_cont.captures(line) {
                    if let Some(seq) = last_seq {
                        map.insert(seq, cap[1].to_string());
                    }
                }
            }
            map
        };

        let mut results = Vec::new();
        for (seq, main_pv) in &main_matches {
            if let Some(suffix) = cont_suffixes.get(seq) {
                results.push(format!("{}{}", main_pv, suffix));
            } else {
                results.push(main_pv.clone());
            }
        }
        return results;
    }

    let mut results = Vec::new();
    let re_gaode = Regex::new(
        r"(?m)^\d+\s+(\S+)\s+(\S+)\s+\d{4}-\d{2}-\d{2}"
    ).unwrap();
    for cap in re_gaode.captures_iter(all_text) {
        results.push(format!("{}{}", &cap[1], &cap[2]));
    }

    results
}

fn extract_reference_times_ordered(all_text: &str) -> Vec<String> {
    let mut results = Vec::new();

    let re_main = Regex::new(
        r"(?m)^(\d+)\s+\S+\s+(\d{2}-\d{2})\s+(\d{1,2})(:\d{2})?[:：]?"
    ).unwrap();
    let re_cont_min = Regex::new(
        r"^(?:\S+\s+)?(\d{1,2})\s+\S+\s+\S"
    ).unwrap();

    let lines: Vec<&str> = all_text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if let Some(cap) = re_main.captures(lines[i]) {
            let date = &cap[2];
            let hour = &cap[3];
            let mut minutes = cap.get(4).map(|m| m.as_str().to_string());
            if minutes.is_none() && i + 1 < lines.len() {
                if let Some(cm) = re_cont_min.captures(lines[i + 1]) {
                    let m = &cm[1];
                    if m.len() <= 2 && m.parse::<u32>().is_ok() {
                        minutes = Some(format!(":{}", m));
                    }
                }
            }
            match minutes {
                Some(m) => results.push(format!("{} {}{}", date, hour, m)),
                None => results.push(format!("{} {}:??", date, hour)),
            }
        }
        i += 1;
    }

    if !results.is_empty() {
        return results;
    }

    let re_gaode = Regex::new(
        r"(?m)^\d+\s+\S+\s+\S+\s+(\d{4}-\d{2}-\d{2})\s+(\d{2}:\d{2})"
    ).unwrap();
    for cap in re_gaode.captures_iter(all_text) {
        results.push(format!("{} {}", &cap[1], &cap[2]));
    }

    results
}

fn is_time_garbled(dt: &str) -> bool {
    let re_ok = Regex::new(r"\d{2,4}-\d{2}-\d{2}").unwrap();
    !re_ok.is_match(dt)
}

fn extract_pickup(text: &str) -> String {
    let re_tft = Regex::new(r"进站[：:]\s*([^\s~出]+)").unwrap();
    if let Some(c) = re_tft.captures(text) {
        return c[1].to_string();
    }
    text.trim().to_string()
}

fn extract_dropoff(text: &str) -> String {
    let re_tft_full = Regex::new(r"出站[：:]\s*([^\s~进]+)").unwrap();
    let re_tft_short = Regex::new(r"(?:^|[^进出])站[：:]\s*([^\s~进]+)").unwrap();
    if let Some(c) = re_tft_full.captures(text) {
        return c[1].to_string();
    }
    if let Some(c) = re_tft_short.captures(text) {
        return c[1].to_string();
    }
    text.trim().to_string()
}

fn find_header(positioned: &[PositionedText]) -> Option<Vec<&PositionedText>> {
    // 找到包含"序号"的块，用其 Y 坐标定位表头行
    let seq_block = positioned.iter().find(|p| p.text.contains("序号"))?;
    let header_y = seq_block.y;

    // 收集 Y 坐标相近的所有块（±20像素视为同一行）
    let header: Vec<&PositionedText> = positioned
        .iter()
        .filter(|p| (p.y - header_y).abs() <= 20.0)
        .collect();

    // 确认表头行包含至少"序号"和另一个关键列名
    let text: String = header.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("");
    if text.contains("序号")
        && (text.contains("时间") || text.contains("金额") || text.contains("起点"))
    {
        Some(header)
    } else {
        None
    }
}

fn estimate_col_span_from_header(header: &[&PositionedText]) -> Option<f64> {
    if header.len() < 2 {
        return None;
    }
    let mut xs: Vec<f64> = header.iter().map(|p| p.x).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let diffs: Vec<f64> = xs.windows(2).map(|w| w[1] - w[0]).filter(|d| *d > 10.0).collect();
    if diffs.is_empty() {
        return None;
    }
    let min_diff = diffs.iter().cloned().fold(f64::INFINITY, f64::min);
    Some(min_diff * 0.8)
}

fn estimate_col_span(data: &[&PositionedText]) -> f64 {
    if data.len() < 2 {
        return 200.0;
    }
    let xs: Vec<f64> = data.iter().map(|p| p.x).collect();
    let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let header_cols = xs.iter().filter(|x| **x > min_x + 50.0).count().max(6);
    (max_x - min_x) / header_cols as f64
}

fn estimate_row_height(data: &[&PositionedText], seq_x: f64, col_span: f64) -> f64 {
    let seq_texts: Vec<f64> = data
        .iter()
        .filter(|p| (p.x - seq_x).abs() <= col_span * 0.5 && is_seq_number(&p.text))
        .map(|p| p.y)
        .collect();
    if seq_texts.len() >= 2 {
        let mut ys = seq_texts;
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let diffs: Vec<f64> = ys.windows(2).map(|w| w[1] - w[0]).filter(|d| *d > 10.0).collect();
        if !diffs.is_empty() {
            return diffs.iter().cloned().fold(f64::INFINITY, f64::min);
        }
    }
    let mut ys: Vec<f64> = data.iter().map(|p| p.y).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let diffs: Vec<f64> = ys.windows(2).map(|w| w[1] - w[0]).filter(|d| *d > 15.0).collect();
    if diffs.is_empty() {
        return 50.0;
    }
    diffs.iter().cloned().fold(f64::INFINITY, f64::min)
}

/// 在表头块中找包含指定关键词的列 X 坐标
/// 优先匹配更长的关键词（更具体），避免"上车"匹配到"上车时间"
fn find_col_x(header: &[&PositionedText], keywords: &[&str]) -> Option<f64> {
    let mut sorted: Vec<&&str> = keywords.iter().collect();
    sorted.sort_by(|a, b| b.len().cmp(&a.len()));
    for kw in sorted {
        for p in header {
            if p.text.contains(*kw) {
                return Some(p.x);
            }
        }
    }
    None
}

/// 天府通行程单解析（按行逐条匹配）
///
/// parangi 提取的天府通文本，每条行程占一行（视觉上是两行但 parangi 合并），
/// 格式变化大：金额和出站可能粘连（3出站： / 5站：），也可能有空格/波浪线分隔。
///
/// 已观察到的行格式：
///   进站：省体育馆 ~ 支付宝1 地铁 4458359453167616 2026-04-24 17:58:59 3出站：花牌坊 APP
///   进站：花牌坊 ~ 出 支付宝2 地铁 4463194522398720 2026-04-26 10:58:13 5站：天宇路 APP
///   进站：成都东客站 支付宝1 地铁 4503905174078464 2026-05-10 20:04:43 3~ 出站：牛王庙 APP
///
/// 每行必定包含：进站：XX、时间、金额、出站/站：YY
fn parse_tianfutong_format(all_text: &str) -> Vec<Itinerary> {
    let re_line = Regex::new(
        r"进站[：:]\s*(\S+).*?(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}).*?(\d+(?:\.\d+)?)\s*(?:~\s*)?(?:出站|站)[：:]\s*(\S+)"
    ).unwrap();

    let mut entries = Vec::new();
    for cap in re_line.captures_iter(all_text) {
        let amount: f64 = cap[3].parse().unwrap_or(0.0);
        if amount > 0.0 {
            entries.push(Itinerary {
                date_time: cap[2].to_string(),
                provider: "天府通".to_string(),
                pickup: cap[1].to_string(),
                dropoff: cap[4].to_string(),
                amount,
            });
        }
    }
    entries
}

fn extract_trailing_numbers(line: &str) -> Vec<f64> {
    let re = Regex::new(r"\b([\d.]+)\b").unwrap();
    let mut numbers = Vec::new();
    for cap in re.find_iter(line) {
        if let Ok(n) = cap.as_str().parse::<f64>() {
            numbers.push(n);
        }
    }
    numbers
}

fn parse_fallback_format(text: &str) -> Vec<Itinerary> {
    let mut results = Vec::new();
    let re_amount = Regex::new(r"[¥￥]\s*([\d.]+)").unwrap();
    let re_time = Regex::new(r"(\d{2}:\d{2})").unwrap();

    for line in text.lines() {
        if let Some(amt) = re_amount.captures(line) {
            let amount: f64 = amt[1].parse().unwrap_or(0.0);
            if amount > 0.0 {
                let time = re_time
                    .captures(line)
                    .map(|c| c[1].to_string())
                    .unwrap_or_default();
                results.push(Itinerary {
                    date_time: time,
                    provider: String::new(),
                    pickup: String::new(),
                    dropoff: String::new(),
                    amount,
                });
            }
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_item(text: &str) -> OcrTextItem {
        OcrTextItem {
            text: text.to_string(),
            confidence: 0.9,
            box_coords: None,
        }
    }

    fn make_positioned_item(text: &str, cx: f64, cy: f64) -> OcrTextItem {
        OcrTextItem {
            text: text.to_string(),
            confidence: 0.9,
            box_coords: Some(serde_json::json!({
                "points": [
                    {"x": cx - 20.0, "y": cy - 5.0},
                    {"x": cx + 20.0, "y": cy - 5.0},
                    {"x": cx + 20.0, "y": cy + 5.0},
                    {"x": cx - 20.0, "y": cy + 5.0}
                ]
            })),
        }
    }

    #[test]
    fn test_parse_standard_format() {
        let texts = vec![
            make_text_item("2025-08-05 09:30  滴滴出行  ¥35.00"),
            make_text_item("2025-08-06 14:20  高德打车  ¥28.50"),
        ];
        let result = parse_itinerary_text(&texts);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].amount, 35.0);
        assert_eq!(result[0].provider, "滴滴出行");
        assert_eq!(result[1].amount, 28.5);
    }

    #[test]
    fn test_parse_fallback_format() {
        let texts = vec![
            make_text_item("行程1 09:30 ¥35.00"),
            make_text_item("行程2 14:20 ¥28.50"),
        ];
        let result = parse_itinerary_text(&texts);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_tianfutong_with_coords() {
        let texts = vec![
            make_positioned_item("天府通 - 行程单", 300.0, 20.0),
            make_positioned_item("序号", 30.0, 120.0),
            make_positioned_item("行程类型", 100.0, 120.0),
            make_positioned_item("出行时间", 300.0, 120.0),
            make_positioned_item("进出站/线路", 500.0, 120.0),
            make_positioned_item("金额(元)", 700.0, 120.0),
            make_positioned_item("进站：成都东客站", 500.0, 180.0),
            make_positioned_item("2026-05-10 20:04:43", 300.0, 180.0),
            make_positioned_item("3~", 700.0, 180.0),
            make_positioned_item("出站：牛王庙", 500.0, 210.0),
        ];
        let result = parse_itinerary_with_coords(&texts);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].provider, "天府通");
        assert_eq!(result[0].date_time, "2026-05-10 20:04:43");
        assert_eq!(result[0].pickup, "成都东客站");
        assert_eq!(result[0].dropoff, "牛王庙");
        assert!((result[0].amount - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_tianfutong_fallback_without_coords() {
        let texts = vec![
            make_text_item("天府通电子行程单"),
            make_text_item("进站：省体育馆~ 2026-04-24 17:58:59 3"),
            make_text_item("出站：花牌坊 APP"),
        ];
        let result = parse_itinerary_with_coords(&texts);
        assert!(!result.is_empty());
        assert_eq!(result[0].provider, "天府通");
    }

    #[test]
    fn test_tianfutong_merged_line() {
        let texts = vec![
            make_text_item("天府通 - 行程单"),
            make_text_item("进站：成都东客站 支付宝1 地铁 4503905174078464 2026-05-10 20:04:43 3~ 出站：牛王庙 APP"),
        ];
        let result = parse_itinerary_text(&texts);
        assert!(!result.is_empty());
        assert_eq!(result[0].provider, "天府通");
        assert_eq!(result[0].pickup, "成都东客站");
        assert_eq!(result[0].dropoff, "牛王庙");
        assert!((result[0].amount - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_tianfutong_multi_trip_stuck() {
        let texts = vec![
            make_text_item("天府通 - 行程单"),
            make_text_item("进站：省体育馆 ~ 支付宝1 地铁 4458359453167616 2026-04-24 17:58:59 3出站：花牌坊 APP"),
            make_text_item("进站：花牌坊 ~ 出 支付宝2 地铁 4463194522398720 2026-04-26 10:58:13 5站：天宇路 APP"),
            make_text_item("进站：天宇路 ~ 出 支付宝3 地铁 4464233532473344 2026-04-26 19:46:41 5站：花牌坊 APP"),
        ];
        let result = parse_itinerary_text(&texts);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].pickup, "省体育馆");
        assert_eq!(result[0].dropoff, "花牌坊");
        assert!((result[0].amount - 3.0).abs() < 0.01);
        assert_eq!(result[1].pickup, "花牌坊");
        assert_eq!(result[1].dropoff, "天宇路");
        assert!((result[1].amount - 5.0).abs() < 0.01);
        assert_eq!(result[2].pickup, "天宇路");
        assert_eq!(result[2].dropoff, "花牌坊");
        assert!((result[2].amount - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_extract_coords() {
        let item = make_positioned_item("测试", 100.0, 200.0);
        let (cx, cy) = extract_coords(&item.box_coords).unwrap();
        assert!((cx - 100.0).abs() < 0.1);
        assert!((cy - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_extract_coords_none() {
        let item = make_text_item("无坐标");
        assert!(extract_coords(&item.box_coords).is_none());
    }

    #[test]
    fn test_enrich_year_from_header_period() {
        // 行程单顶部"行程时间：2026年4月"，行程条目无年份 "04-22 21:30"
        let mut entries = vec![
            Itinerary { date_time: "04-22 21:30".to_string(), provider: "滴滴".to_string(), pickup: "A".to_string(), dropoff: "B".to_string(), amount: 35.0 },
            Itinerary { date_time: "04-25 08:48".to_string(), provider: "滴滴".to_string(), pickup: "C".to_string(), dropoff: "D".to_string(), amount: 40.0 },
        ];
        let all_text = "滴滴出行行程单\n行程时间：2026年4月\n1 专车 04-22 21:30 成都 35.00\n2 专车 04-25 08:48 成都 40.00";
        enrich_itinerary_years(&mut entries, all_text);
        assert_eq!(entries[0].date_time, "2026-04-22 21:30");
        assert_eq!(entries[1].date_time, "2026-04-25 08:48");
    }

    #[test]
    fn test_enrich_year_skips_already_dated() {
        let mut entries = vec![
            Itinerary { date_time: "2026-04-22 21:30".to_string(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 35.0 },
        ];
        enrich_itinerary_years(&mut entries, "2026年4月");
        assert_eq!(entries[0].date_time, "2026-04-22 21:30");
    }

    #[test]
    fn test_enrich_year_no_year_in_text_keeps_original() {
        let mut entries = vec![
            Itinerary { date_time: "04-22 21:30".to_string(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 35.0 },
        ];
        enrich_itinerary_years(&mut entries, "滴滴行程单\n无年份信息");
        assert_eq!(entries[0].date_time, "04-22 21:30");
    }

    #[test]
    fn test_enrich_year_from_iso_date_in_text() {
        // 顶部有 "2026-04-22 至 2026-04-25" 区间
        let mut entries = vec![
            Itinerary { date_time: "04-25 08:48".to_string(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 40.0 },
        ];
        enrich_itinerary_years(&mut entries, "行程时间 2026-04-22 至 2026-04-25");
        assert_eq!(entries[0].date_time, "2026-04-25 08:48");
    }
}
