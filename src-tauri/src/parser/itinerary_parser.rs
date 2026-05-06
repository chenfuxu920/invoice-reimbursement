use crate::models::invoice::Itinerary;
use crate::ocr::client::OcrTextItem;
use regex::Regex;

pub fn parse_itinerary_text(texts: &[OcrTextItem]) -> Vec<Itinerary> {
    let all_text: String = texts
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let mut itineraries = Vec::new();

    // 常见行程单格式：2025-08-05 09:30  滴滴出行  ¥35.00
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

    // 如果无法匹配标准格式，尝试其他格式
    if itineraries.is_empty() {
        itineraries = parse_fallback_format(&all_text);
    }

    itineraries
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
