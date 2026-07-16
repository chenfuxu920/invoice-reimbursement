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

/// 合并换行拆分的时间：将 "MM-DD HH:\n  MM" 合并为 "MM-DD HH:MM"
/// 解决滴滴行程单中时间列单元格内换行导致分钟丢失的问题
fn merge_split_times(text: &str) -> String {
    // 模式1：word-level atomized text — "06-07\n20:\n17" → "06-07 20:17"
    let re_word = Regex::new(r"(?m)^(\d{2}-\d{2})\s*\n\s*(\d{1,2}[:：])\s*\n\s*(\d{1,2})\b").unwrap();
    let merged = re_word.replace_all(text, "$1 $2$3").to_string();

    // 模式2：主行 "06-07 20:" 续行 "17 周日" → 合并为 "06-07 20:17 周日"
    let re = Regex::new(r"(?m)(\d{2}-\d{2}\s+\d{1,2}:)\s*\n\s*(\d{1,2})(\s|$)").unwrap();
    re.replace_all(&merged, "$1$2$3").to_string()
}

fn parse_itinerary_text_impl(all_text: &str) -> Vec<Itinerary> {
    let mut itineraries = Vec::new();

    // 预处理：合并换行拆分的时间
    let all_text = merge_split_times(all_text);

    // 格式1：OCR 输出，带 ¥ 符号  2025-08-05 09:30  滴滴出行  ¥35.00
    let re = Regex::new(
        r"(?m)(\d{4}[-/]\d{2}[-/]\d{2}\s+\d{2}:\d{2})\s+(.+?)\s+[¥￥]\s*([\d.]+)",
    )
    .unwrap();

    for cap in re.captures_iter(&all_text) {
        itineraries.push(Itinerary {
            date_time: cap[1].to_string(),
            provider: cap[2].trim().to_string(),
            pickup: String::new(),
            dropoff: String::new(),
            amount: cap[3].parse().unwrap_or(0.0),
            incomplete_fields: Vec::new(),
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
    let re_cont_min = Regex::new(r"^(\d{1,2})\b").unwrap();

    let lines: Vec<&str> = all_text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(cap) = re_table.captures(line) {
            let _seq: u32 = cap[1].parse().unwrap_or(0);
            let mut date_time = cap[2].trim().to_string();

            // ponytail: 修复换行时间 — 当捕获的时间以冒号结尾时，
            // 检查下一行是否以分钟数字开头（滴滴行程单续行分钟）
            if (date_time.ends_with(':') || date_time.ends_with('：')) && i + 1 < lines.len() {
                let next = lines[i + 1].trim();
                if let Some(m) = re_cont_min.captures(next) {
                    let mins = &m[1];
                    if mins.parse::<u32>().map_or(false, |n| n < 60) {
                        date_time = format!("{}{}", date_time, mins);
                    }
                }
            }

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
                        incomplete_fields: Vec::new(),
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
        let tft_entries = parse_tianfutong_format(&all_text);
        if !tft_entries.is_empty() {
            return tft_entries;
        }
    }

    if !itineraries.is_empty() {
        return itineraries;
    }

    // 格式4：回退（无匹配）
    itineraries
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

    let header_col_span = estimate_col_span_from_header(&header);
    let data_col_span = estimate_col_span(&data);
    let col_span = header_col_span.map_or(data_col_span, |h| h.max(data_col_span));
    let seq_x = col_map.iter().find(|(s, _)| *s == SemanticCol::Seq).map(|(_, x)| *x);
    let avg_row_h = estimate_row_height(&data, seq_x.unwrap_or(0.0), col_span);

    let time_x = col_map.iter().find(|(s, _)| *s == SemanticCol::Time).map(|(_, x)| *x);

    let re_datetime_full = Regex::new(r"(\d{4}-\d{2}-\d{2})\s*(\d{2}:\d{2}:\d{2})").unwrap();
    let re_datetime_full_nosec = Regex::new(r"(\d{4}-\d{2}-\d{2})\s*(\d{1,2}:\d{2})").unwrap();
    let re_datetime_full_nospace = Regex::new(r"(\d{4}-\d{2}-\d{2})(\d{1,2}:\d{2})").unwrap();
    let re_datetime_short = Regex::new(r"(\d{2}-\d{2})\s*(\d{1,2}:\d{2})").unwrap();
    let re_datetime_colon_cont = Regex::new(r"(\d{2}-\d{2})\s*(\d{1,2})[:：]\s+(\d{2})").unwrap();
    let re_datetime_mins_cont = Regex::new(r"(\d{2}-\d{2})(\d{1,2})\s+(\d{2})").unwrap();
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
            // ponytail: 用 next_y - tolerance 代替中点——
            // 续行可能在两行锚点中偏上的位置，中点位容易漏掉
            next_y - avg_row_h * 0.3
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
            } else if let Some(c) = re_datetime_colon_cont.captures(&time_text) {
                format!("{} {}:{}", &c[1], &c[2], &c[3])
            } else if let Some(c) = re_datetime_mins_cont.captures(&time_text) {
                format!("{} {}:{}", &c[1], &c[2], &c[3])
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
        // ponytail: OCR 有时丢失金额小数点（"30.60"→"3060"），>=1000 且无小数点时除以 100
        let amount = if amount >= 1000.0 && !amount_text.contains('.') { amount / 100.0 } else { amount };

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
                incomplete_fields: Vec::new(),
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

/// 检查单条行程提取结果是否不完整（有字段未提取到）
fn itinerary_is_incomplete(entry: &Itinerary) -> bool {
    if entry.amount <= 0.0 {
        return true;
    }
    let dt = entry.date_time.trim();
    if dt.is_empty() {
        return true;
    }
    if dt.contains("??") {
        return true;
    }
    if dt.ends_with(':') || dt.ends_with('：') {
        return true;
    }
    if !dt.contains(':') && !dt.contains('：') {
        return true;
    }
    false
}

/// 任意一条行程有未提取到的字段时返回 true
pub fn has_incomplete_entries(entries: &[Itinerary]) -> bool {
    entries.iter().any(|e| itinerary_is_incomplete(e))
}

/// 计算并填充每条行程的 incomplete_fields 列表
pub fn compute_incomplete_fields(entries: &mut [Itinerary]) {
    for entry in entries {
        let mut missing = Vec::new();
        if entry.amount <= 0.0 {
            missing.push("amount".to_string());
        }
        let dt = entry.date_time.trim();
        if dt.is_empty() || dt.contains("??") || dt.ends_with(':') || dt.ends_with('：')
            || (!dt.contains(':') && !dt.contains('：'))
        {
            missing.push("date_time".to_string());
        }
        entry.incomplete_fields = missing;
    }
}

fn looks_like_datetime(text: &str) -> bool {
    let re_full = Regex::new(r"^\d{4}-\d{2}-\d{2}").unwrap();
    let re_short_digit = Regex::new(r"^\d{2}-\d{2}\d").unwrap();
    let re_short_colon = Regex::new(r"^\d{2}-\d{2}\s*\d{0,2}[:：]").unwrap();
    // ponytail: bare "MM-DD" date — pdfplumber splits "06-23" and "09:" into separate words
    let re_short_bare = Regex::new(r"^\d{2}-\d{2}$").unwrap();
    re_full.is_match(text) || re_short_digit.is_match(text) || re_short_colon.is_match(text) || re_short_bare.is_match(text)
}

fn clean_time_text(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.iter().any(|w| is_seq_number(w)) {
        let mut filtered = Vec::new();
        for (i, w) in words.iter().enumerate() {
            if is_seq_number(w) {
                // ponytail: only strip pure-numeric seq numbers, not mixed-content like "46周五"
                let is_pure_num = w.trim().parse::<u32>().is_ok();
                // keep if follows ':' — could be minutes from continuation line
                if i > 0 && (words[i - 1].ends_with(':') || words[i - 1].ends_with('：')) {
                    filtered.push(*w);
                } else if is_pure_num {
                    continue;
                } else {
                    filtered.push(*w);
                }
                continue;
            }
            filtered.push(*w);
        }
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
/// 当月份回退（如 12→1）时自动递增年份，处理跨年行程。
pub fn enrich_itinerary_years(entries: &mut [Itinerary], all_text: &str) {
    let base_year = match extract_year_from_text(all_text) {
        Some(y) => y,
        None => return,
    };
    // 仅匹配以 "MM-DD" 开头（无年份）的 date_time。
    // "YYYY-MM-DD" 因第3字符非 '-' 不会被误匹配。
    let re_no_year = Regex::new(r"^(\d{2})-(\d{2})(.*)").unwrap();
    let mut current_year = base_year;
    let mut prev_month: u32 = 0;
    for entry in entries.iter_mut() {
        if let Some(cap) = re_no_year.captures(&entry.date_time) {
            let month: u32 = cap[1].parse().unwrap_or(0);
            // 月份回退（如 12→1），说明跨年了
            if month > 0 && month < prev_month {
                current_year += 1;
            }
            entry.date_time = format!("{}-{}-{}{}", current_year, &cap[1], &cap[2], &cap[3]);
            prev_month = month;
        }
    }
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
                incomplete_fields: Vec::new(),
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
            Itinerary { date_time: "04-22 21:30".to_string(), provider: "滴滴".to_string(), pickup: "A".to_string(), dropoff: "B".to_string(), amount: 35.0, incomplete_fields: vec![] },
            Itinerary { date_time: "04-25 08:48".to_string(), provider: "滴滴".to_string(), pickup: "C".to_string(), dropoff: "D".to_string(), amount: 40.0, incomplete_fields: vec![] },
        ];
        let all_text = "滴滴出行行程单\n行程时间：2026年4月\n1 专车 04-22 21:30 成都 35.00\n2 专车 04-25 08:48 成都 40.00";
        enrich_itinerary_years(&mut entries, all_text);
        assert_eq!(entries[0].date_time, "2026-04-22 21:30");
        assert_eq!(entries[1].date_time, "2026-04-25 08:48");
    }

    #[test]
    fn test_enrich_year_skips_already_dated() {
        let mut entries = vec![
            Itinerary { date_time: "2026-04-22 21:30".to_string(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 35.0, incomplete_fields: vec![] },
        ];
        enrich_itinerary_years(&mut entries, "2026年4月");
        assert_eq!(entries[0].date_time, "2026-04-22 21:30");
    }

    #[test]
    fn test_enrich_year_no_year_in_text_keeps_original() {
        let mut entries = vec![
            Itinerary { date_time: "04-22 21:30".to_string(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 35.0, incomplete_fields: vec![] },
        ];
        enrich_itinerary_years(&mut entries, "滴滴行程单\n无年份信息");
        assert_eq!(entries[0].date_time, "04-22 21:30");
    }

    #[test]
    fn test_enrich_year_from_iso_date_in_text() {
        // 顶部有 "2026-04-22 至 2026-04-25" 区间
        let mut entries = vec![
            Itinerary { date_time: "04-25 08:48".to_string(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 40.0, incomplete_fields: vec![] },
        ];
        enrich_itinerary_years(&mut entries, "行程时间 2026-04-22 至 2026-04-25");
        assert_eq!(entries[0].date_time, "2026-04-25 08:48");
    }

    #[test]
    fn test_enrich_year_cross_year_by_month_rollback() {
        // 跨年行程：12-28 → 12-30 → 01-02
        // 月份从12降到1，说明跨年了，1月的条目应该年份+1
        let mut entries = vec![
            Itinerary { date_time: "12-28 21:30".to_string(), provider: "滴滴".to_string(), pickup: "A".to_string(), dropoff: "B".to_string(), amount: 35.0, incomplete_fields: vec![] },
            Itinerary { date_time: "12-30 08:48".to_string(), provider: "滴滴".to_string(), pickup: "C".to_string(), dropoff: "D".to_string(), amount: 40.0, incomplete_fields: vec![] },
            Itinerary { date_time: "01-02 09:00".to_string(), provider: "滴滴".to_string(), pickup: "E".to_string(), dropoff: "F".to_string(), amount: 45.0, incomplete_fields: vec![] },
        ];
        enrich_itinerary_years(&mut entries, "行程时间：2025年12月-2026年1月");
        assert_eq!(entries[0].date_time, "2025-12-28 21:30", "12月应使用基准年2025");
        assert_eq!(entries[1].date_time, "2025-12-30 08:48", "同月不递增");
        assert_eq!(entries[2].date_time, "2026-01-02 09:00", "月回退12→1，年份+1");
    }

    #[test]
    fn test_enrich_year_no_rollback_when_same_year() {
        // 同一年内月份递增：03-15 → 04-01 → 05-20
        let mut entries = vec![
            Itinerary { date_time: "03-15 10:00".to_string(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 30.0, incomplete_fields: vec![] },
            Itinerary { date_time: "04-01 14:00".to_string(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 35.0, incomplete_fields: vec![] },
            Itinerary { date_time: "05-20 09:00".to_string(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 40.0, incomplete_fields: vec![] },
        ];
        enrich_itinerary_years(&mut entries, "行程时间：2026年3月-5月");
        assert_eq!(entries[0].date_time, "2026-03-15 10:00");
        assert_eq!(entries[1].date_time, "2026-04-01 14:00");
        assert_eq!(entries[2].date_time, "2026-05-20 09:00");
    }

    #[test]
    fn test_clean_time_keeps_minutes_after_colon() {
        let result = clean_time_text("06-23 09: 39 周二");
        assert_eq!(result, "06-23 09: 39 周二");
    }

    #[test]
    fn test_clean_time_strips_seq_at_start() {
        let result = clean_time_text("3 06-23 09:39");
        assert_eq!(result, "06-23 09:39");
    }

    #[test]
    fn test_didi_split_time_with_coords() {
        let texts = vec![
            make_positioned_item("序号", 50.0, 120.0),
            make_positioned_item("车型", 120.0, 120.0),
            make_positioned_item("上车时间", 200.0, 120.0),
            make_positioned_item("城市", 350.0, 120.0),
            make_positioned_item("起点", 450.0, 120.0),
            make_positioned_item("终点", 650.0, 120.0),
            make_positioned_item("里程", 850.0, 120.0),
            make_positioned_item("金额", 950.0, 120.0),
            make_positioned_item("1", 50.0, 180.0),
            make_positioned_item("专车", 120.0, 180.0),
            make_positioned_item("06-23 09:", 200.0, 180.0),
            make_positioned_item("长沙", 350.0, 180.0),
            make_positioned_item("A小区", 450.0, 180.0),
            make_positioned_item("机场", 650.0, 180.0),
            make_positioned_item("41.7", 850.0, 180.0),
            make_positioned_item("51.30", 950.0, 180.0),
            make_positioned_item("39", 200.0, 215.0),
            make_positioned_item("周二", 200.0, 230.0),
            make_positioned_item("市", 350.0, 215.0),
        ];
        let result = parse_itinerary_with_coords(&texts);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].date_time, "06-23 09:39");
        assert!((result[0].amount - 51.30).abs() < 0.01);
        assert_eq!(result[0].provider, "专车");
    }

    #[test]
    fn test_merged_datehour_with_mins_continuation() {
        let texts = vec![
            make_positioned_item("序号", 50.0, 120.0),
            make_positioned_item("车型", 120.0, 120.0),
            make_positioned_item("上车时间", 200.0, 120.0),
            make_positioned_item("里程", 850.0, 120.0),
            make_positioned_item("金额", 950.0, 120.0),
            make_positioned_item("3", 50.0, 180.0),
            make_positioned_item("专车", 120.0, 180.0),
            make_positioned_item("07-0320", 200.0, 180.0),
            make_positioned_item("14.1", 850.0, 180.0),
            make_positioned_item("52.10", 950.0, 180.0),
            make_positioned_item("46周五", 200.0, 215.0),
        ];
        let result = parse_itinerary_with_coords(&texts);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].date_time, "07-03 20:46");
        assert!((result[0].amount - 52.10).abs() < 0.01);
    }

    #[test]
    fn test_complete_entry_is_valid() {
        let entries = vec![
            Itinerary { date_time: "2026-06-23 09:39".into(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 51.30, incomplete_fields: vec![] },
        ];
        assert!(!has_incomplete_entries(&entries));
    }

    #[test]
    fn test_empty_list_is_complete() {
        let entries: Vec<Itinerary> = vec![];
        assert!(!has_incomplete_entries(&entries));
    }

    #[test]
    fn test_missing_minutes_is_incomplete() {
        let entries = vec![
            Itinerary { date_time: "2026-06-23 09:??".into(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 51.30, incomplete_fields: vec![] },
        ];
        assert!(has_incomplete_entries(&entries));
    }

    #[test]
    fn test_trailing_colon_is_incomplete() {
        let entries = vec![
            Itinerary { date_time: "2026-04-30 08:".into(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 100.0, incomplete_fields: vec![] },
        ];
        assert!(has_incomplete_entries(&entries));
    }

    #[test]
    fn test_date_only_no_time_is_incomplete() {
        let entries = vec![
            Itinerary { date_time: "2026-06-23".into(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 51.30, incomplete_fields: vec![] },
        ];
        assert!(has_incomplete_entries(&entries));
    }

    #[test]
    fn test_zero_amount_is_incomplete() {
        let entries = vec![
            Itinerary { date_time: "2026-06-23 09:39".into(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 0.0, incomplete_fields: vec![] },
        ];
        assert!(has_incomplete_entries(&entries));
    }

    #[test]
    fn test_empty_time_is_incomplete() {
        let entries = vec![
            Itinerary { date_time: String::new(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 10.0, incomplete_fields: vec![] },
        ];
        assert!(has_incomplete_entries(&entries));
    }

    #[test]
    fn test_mixed_complete_and_incomplete() {
        let entries = vec![
            Itinerary { date_time: "2026-06-23 09:39".into(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 51.30, incomplete_fields: vec![] },
            Itinerary { date_time: "2026-07-03 17:??".into(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 114.10, incomplete_fields: vec![] },
        ];
        assert!(has_incomplete_entries(&entries));
    }

    #[test]
    fn test_compute_incomplete_fields_flags_missing_time() {
        let mut entries = vec![
            Itinerary { date_time: "2026-06-23 09:??".into(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 51.30, incomplete_fields: vec![] },
        ];
        compute_incomplete_fields(&mut entries);
        assert!(entries[0].incomplete_fields.contains(&"date_time".to_string()));
        assert!(!entries[0].incomplete_fields.contains(&"amount".to_string()));
    }

    #[test]
    fn test_compute_incomplete_fields_ok_when_complete() {
        let mut entries = vec![
            Itinerary { date_time: "2026-06-23 09:39".into(), provider: String::new(), pickup: String::new(), dropoff: String::new(), amount: 51.30, incomplete_fields: vec![] },
        ];
        compute_incomplete_fields(&mut entries);
        assert!(entries[0].incomplete_fields.is_empty());
    }

    // === 换行时间合并回归测试 ===

    #[test]
    fn test_merge_split_times_basic() {
        // 滴滴行程单时间列单元格内折行： "06-07 20:\n  17 周日"
        let input = "1 专车 06-07 20:\n17 周日\n长沙\n芙蓉北路\n60.6\n195.37";
        let merged = merge_split_times(input);
        assert!(merged.contains("06-07 20:17"), "应合并为 06-07 20:17，实际: {merged}");
        // ponytail: 冒号后仍有 "20:17"，contains("20:") 永远为真，只断言合并结果
    }

    #[test]
    fn test_merge_split_times_no_merge_when_complete() {
        // 完整时间不应改动
        let input = "04-22 21:30 成都 ¥100.00";
        let merged = merge_split_times(input);
        assert_eq!(merged, input);
    }

    #[test]
    fn test_parse_fallback_split_time_no_mins() {
        // 无冒号结尾的时间 — 不应错误合并
        let input = "06-07\n无分钟\n¥35.00";
        let merged = merge_split_times(input);
        // 不应错误合并（缺少 "HH:" 冒号模式）
        assert_eq!(merged, input);
    }

    #[test]
    fn test_format2_parangi_split_time() {
        // parangi 文本格式：主行有 "06-07 20: 长沙"，分钟 "17" 在续行
        let texts = vec![
            make_text_item("1 专车 06-07 20: 长沙 芙蓉北路|... 60.6 195.37"),
            make_text_item("       17 周日"),
            make_text_item("2 专车 06-12 21: 长沙 长沙火车站 45.2 114.10"),
            make_text_item("       41 周五"),
        ];
        let result = parse_itinerary_text(&texts);
        assert_eq!(result.len(), 2, "应解析出 2 条行程");
        // 时间不应以冒号结尾
        assert_eq!(result[0].date_time, "06-07 20:17");
        assert_eq!(result[1].date_time, "06-12 21:41");
        assert_eq!(result[0].amount, 195.37);
        assert_eq!(result[1].amount, 114.10);
    }

    #[test]
    fn test_merge_split_times_word_level() {
        // word-level atomized text: "06-07\n20:\n17" → "06-07 20:17"
        let input = "1\n专车\n06-07\n20:\n17\n周日\n长沙\n¥195.37\n2\n专车\n06-12\n21:\n41\n周五\n长沙\n¥114.10";
        let merged = merge_split_times(input);
        assert!(merged.contains("06-07 20:17"), "word-level 应合并为 06-07 20:17，实际: {merged}");
        assert!(merged.contains("06-12 21:41"), "word-level 应合并为 06-12 21:41");
    }

    #[test]
    fn test_two_row_split_time_tight_gap() {
        // 两行行程，行间距仅 30，续行紧靠中点 — 旧 y_hi 中点逻辑会漏
        // Row1@180, Row2@210, 续行@200, 旧 y_hi=(180+210)/2=195→排除
        // 新 y_hi=210-tolerance=210-9=201→包含
        let texts = vec![
            make_positioned_item("序号", 50.0, 120.0),
            make_positioned_item("车型", 120.0, 120.0),
            make_positioned_item("上车时间", 200.0, 120.0),
            make_positioned_item("城市", 350.0, 120.0),
            make_positioned_item("起点", 450.0, 120.0),
            make_positioned_item("终点", 650.0, 120.0),
            make_positioned_item("里程", 850.0, 120.0),
            make_positioned_item("金额", 950.0, 120.0),
            // 行1：主行=180, 续行=200
            make_positioned_item("1", 50.0, 180.0),
            make_positioned_item("专车", 120.0, 180.0),
            make_positioned_item("06-07 20:", 200.0, 180.0),
            make_positioned_item("长沙", 350.0, 180.0),
            make_positioned_item("芙蓉北路", 450.0, 180.0),
            make_positioned_item("机场", 650.0, 180.0),
            make_positioned_item("60.6", 850.0, 180.0),
            make_positioned_item("195.37", 950.0, 180.0),
            make_positioned_item("17", 200.0, 200.0),
            make_positioned_item("周日", 200.0, 204.0),
            make_positioned_item("市", 350.0, 200.0),
            // 行2：主行=210（紧靠续行200）
            make_positioned_item("2", 50.0, 210.0),
            make_positioned_item("专车", 120.0, 210.0),
            make_positioned_item("06-12 21:", 200.0, 210.0),
            make_positioned_item("长沙", 350.0, 210.0),
            make_positioned_item("火车站", 450.0, 210.0),
            make_positioned_item("酒店", 650.0, 210.0),
            make_positioned_item("45.2", 850.0, 210.0),
            make_positioned_item("114.10", 950.0, 210.0),
            make_positioned_item("41", 200.0, 230.0),
            make_positioned_item("周五", 200.0, 234.0),
            make_positioned_item("市", 350.0, 230.0),
        ];
        let result = parse_itinerary_with_coords(&texts);
        assert_eq!(result.len(), 2, "应解析出 2 条行程");
        assert_eq!(result[0].date_time, "06-07 20:17");
        assert_eq!(result[1].date_time, "06-12 21:41");
    }
}
