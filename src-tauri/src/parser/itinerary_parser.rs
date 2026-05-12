use crate::models::invoice::Itinerary;
use crate::ocr::OcrTextItem;
use regex::Regex;

pub fn parse_itinerary_text(texts: &[OcrTextItem]) -> Vec<Itinerary> {
    let all_text: String = texts
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let mut itineraries = Vec::new();

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

    // 先尝试按行匹配
    let lines: Vec<&str> = all_text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(cap) = re_table.captures(line) {
            let _seq: u32 = cap[1].parse().unwrap_or(0);
            let date_time = cap[2].trim().to_string();
            // 从行尾提取两个数字：里程和金额
            let nums = extract_trailing_numbers(line);
            if nums.len() >= 2 {
                let amount = nums[nums.len() - 1]; // 最后一个数是金额
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

    // 格式3：天府通格式
    // 进站：省体育馆~ 支付： 地铁 ... 2026-04-24 17:58:59 3出站：花牌坊 APP
    // 共N笔行程，合计N元
    let re_tft_entry = Regex::new(
        r"(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}).*?(\d+(?:\.\d+)?)(?:出站|站)[：:]\s*(\S+)"
    ).unwrap();

    // 提取每条行程（含单笔金额）
    let mut tft_entries: Vec<Itinerary> = Vec::new();
    for line in all_text.lines() {
        if let Some(cap) = re_tft_entry.captures(line) {
            let per_amount: f64 = cap[2].parse().unwrap_or(0.0);
            tft_entries.push(Itinerary {
                date_time: cap[1].to_string(),
                provider: "天府通".to_string(),
                pickup: String::new(),
                dropoff: cap[3].to_string(),
                amount: per_amount,
            });
        }
    }

    if !tft_entries.is_empty() {
        return tft_entries;
    }

    if !itineraries.is_empty() {
        return itineraries;
    }

    // 格式4：回退，找 ¥ 金额
    parse_fallback_format(&all_text)
}

/// 从行尾提取所有数字
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
}
