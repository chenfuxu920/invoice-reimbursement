use crate::ocr::structured_output::{BoundingBox, OcrStructuredOutput};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedField {
    pub value: String,
    pub confidence: f64,
    pub strategy: String,
    pub source_position: Option<BoundingBox>,
}

pub trait FieldExtractor: Send + Sync {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField>;
}

pub struct RegexStrategy {
    name: String,
    patterns: Vec<Regex>,
}

impl RegexStrategy {
    pub fn new(name: &str, patterns: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            patterns: patterns.iter().filter_map(|p| Regex::new(p).ok()).collect(),
        }
    }
}

impl FieldExtractor for RegexStrategy {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        let text = ocr
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        for pattern in &self.patterns {
            if let Some(caps) = pattern.captures(&text) {
                if let Some(value_match) = caps.get(1) {
                    return Some(ExtractedField {
                        value: value_match.as_str().trim().to_string(),
                        confidence: 0.9,
                        strategy: format!("regex:{}", self.name),
                        source_position: None,
                    });
                }
            }
        }
        None
    }
}

pub struct KeyValueProximityStrategy {
    section_keyword: String,
    field_keyword: String,
}

impl KeyValueProximityStrategy {
    pub fn new(section_keyword: &str, field_keyword: &str) -> Self {
        Self {
            section_keyword: section_keyword.to_string(),
            field_keyword: field_keyword.to_string(),
        }
    }
}

impl FieldExtractor for KeyValueProximityStrategy {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        let section_block = ocr
            .blocks
            .iter()
            .find(|b| b.text.contains(&self.section_keyword))?;

        let page_height = if ocr.layout.height > 0.0 {
            ocr.layout.height
        } else {
            1000.0
        };
        let proximity_threshold = page_height * 0.2;

        let field_block = ocr
            .blocks
            .iter()
            .filter(|b| {
                b.bbox.y > section_block.bbox.y
                    && (b.bbox.y - section_block.bbox.y) < proximity_threshold
            })
            .find(|b| b.text.contains(&self.field_keyword))?;

        let text = &field_block.text;
        if let Some(pos) = text.find(&self.field_keyword) {
            let value_start = pos + self.field_keyword.len();
            let remaining = text[value_start..]
                .trim_start_matches(|c| c == '：' || c == ':' || c == ' ');
            let value = remaining.split_whitespace().next()?.to_string();

            return Some(ExtractedField {
                value,
                confidence: field_block.confidence,
                strategy: format!("proximity:{}:{}", self.section_keyword, self.field_keyword),
                source_position: Some(field_block.bbox.clone()),
            });
        }

        None
    }
}

pub struct ContextualStrategy {
    #[allow(dead_code)]
    invoice_type: String,
}

impl ContextualStrategy {
    pub fn new(invoice_type: &str) -> Self {
        Self {
            invoice_type: invoice_type.to_string(),
        }
    }
}

impl FieldExtractor for ContextualStrategy {
    fn extract(&self, _ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        None
    }
}

pub struct SellerNameExtractor {
    strategies: Vec<Box<dyn FieldExtractor>>,
}

impl SellerNameExtractor {
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Box::new(RegexStrategy::new(
                    "销售方名称",
                    &[
                        r"销售方[：:]\s*名称[：:]\s*(\S+)",
                        r"名称[：:]\s*(\S+)",
                        r"销售方[：:]\s*(\S+)",
                        r"收款单位[：:]\s*(\S+)",
                        r"开票方[：:]\s*(\S+)",
                    ],
                )),
                Box::new(KeyValueProximityStrategy::new("销售方信息", "名称")),
                Box::new(ContextualStrategy::new("增值税发票销售方")),
            ],
        }
    }
}

impl FieldExtractor for SellerNameExtractor {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        for strategy in &self.strategies {
            if let Some(field) = strategy.extract(ocr) {
                return Some(field);
            }
        }
        None
    }
}

pub struct ItemNameExtractor {
    strategies: Vec<Box<dyn FieldExtractor>>,
}

impl ItemNameExtractor {
    pub fn new() -> Self {
        Self {
            strategies: vec![Box::new(RegexStrategy::new(
                "项目名称",
                &[
                    r"(?:项目名称|货物或应税劳务|商品名称|服务名称)[：:]\s*(\S+)",
                    r"品目[：:]\s*(\S+)",
                ],
            ))],
        }
    }
}

impl FieldExtractor for ItemNameExtractor {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        for strategy in &self.strategies {
            if let Some(field) = strategy.extract(ocr) {
                return Some(field);
            }
        }
        None
    }
}

pub struct AmountExtractor {
    strategies: Vec<Box<dyn FieldExtractor>>,
}

impl AmountExtractor {
    pub fn new() -> Self {
        Self {
            strategies: vec![Box::new(RegexStrategy::new(
                "金额",
                &[
                    r"(?:价税合计|合计金额|总金额|金额|实付金额)[^0-9]*([\d,]+\.?\d*)",
                    r"￥\s*([\d,]+\.?\d*)",
                    r"¥\s*([\d,]+\.?\d*)",
                ],
            ))],
        }
    }

    pub fn extract_max_amount(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        let _text = ocr
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let amount_pattern = Regex::new(r"[￥¥]\s*([\d,]+\.?\d*)").ok()?;
        let mut max_amount = 0.0f64;
        let mut max_amount_str = String::new();
        let mut max_confidence = 0.0;

        for block in &ocr.blocks {
            for cap in amount_pattern.captures_iter(&block.text) {
                if let Some(amount_match) = cap.get(1) {
                    let amount_str = amount_match.as_str().replace(",", "");
                    if let Ok(amount) = amount_str.parse::<f64>() {
                        if amount > max_amount {
                            max_amount = amount;
                            max_amount_str = amount_match.as_str().to_string();
                            max_confidence = block.confidence;
                        }
                    }
                }
            }
        }

        if max_amount > 0.0 {
            return Some(ExtractedField {
                value: max_amount_str,
                confidence: max_confidence,
                strategy: "max_amount_fallback".to_string(),
                source_position: None,
            });
        }

        None
    }
}

impl FieldExtractor for AmountExtractor {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        if let Some(field) = self.strategies.first()?.extract(ocr) {
            return Some(field);
        }

        self.extract_max_amount(ocr)
    }
}

pub struct DateExtractor {
    cn_pattern: Regex,
    iso_pattern: Regex,
}

impl DateExtractor {
    pub fn new() -> Self {
        Self {
            cn_pattern: Regex::new(r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日").unwrap(),
            iso_pattern: Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap(),
        }
    }

    pub fn parse_to_date(&self, field: &ExtractedField) -> Option<chrono::NaiveDate> {
        let text = &field.value;

        if let Some(caps) = self.cn_pattern.captures(text) {
            let y: i32 = caps[1].parse().ok()?;
            let m: u32 = caps[2].parse().ok()?;
            let d: u32 = caps[3].parse().ok()?;
            return chrono::NaiveDate::from_ymd_opt(y, m, d);
        }

        if let Some(caps) = self.iso_pattern.captures(text) {
            let y: i32 = caps[1].parse().ok()?;
            let m: u32 = caps[2].parse().ok()?;
            let d: u32 = caps[3].parse().ok()?;
            return chrono::NaiveDate::from_ymd_opt(y, m, d);
        }

        None
    }
}

impl FieldExtractor for DateExtractor {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        let text = ocr
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        if let Some(caps) = self.cn_pattern.captures(&text) {
            let date_str = caps.get(0).unwrap().as_str().to_string();
            return Some(ExtractedField {
                value: date_str,
                confidence: 0.9,
                strategy: "date_cn".to_string(),
                source_position: None,
            });
        }

        if let Some(caps) = self.iso_pattern.captures(&text) {
            let date_str = caps.get(0).unwrap().as_str().to_string();
            return Some(ExtractedField {
                value: date_str,
                confidence: 0.9,
                strategy: "date_iso".to_string(),
                source_position: None,
            });
        }

        None
    }
}

pub struct InvoiceNumberExtractor {
    strategies: Vec<Box<dyn FieldExtractor>>,
}

impl InvoiceNumberExtractor {
    pub fn new() -> Self {
        Self {
            strategies: vec![Box::new(RegexStrategy::new(
                "发票号码",
                &[
                    r"(?:发票号码|发票代码|No|号码)[：:]?\s*(\d+)",
                    r"发票编号[：:]?\s*(\d+)",
                    r"(\d{8,20})\s*(?:发票号码|发票代码|号码)[：:]?",
                ],
            ))],
        }
    }
}

impl FieldExtractor for InvoiceNumberExtractor {
    fn extract(&self, ocr: &OcrStructuredOutput) -> Option<ExtractedField> {
        for strategy in &self.strategies {
            if let Some(field) = strategy.extract(ocr) {
                return Some(field);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::structured_output::{OcrTextBlock, TextBlockType};

    fn create_ocr_output(texts: Vec<&str>) -> OcrStructuredOutput {
        let blocks = texts
            .iter()
            .enumerate()
            .map(|(i, text)| OcrTextBlock {
                text: text.to_string(),
                confidence: 0.95,
                bbox: BoundingBox {
                    x: 0.0,
                    y: (i * 20) as f64,
                    width: 200.0,
                    height: 20.0,
                },
                line_index: i,
                block_type: if text.contains("：") {
                    TextBlockType::KeyValue
                } else {
                    TextBlockType::Other
                },
            })
            .collect();

        OcrStructuredOutput {
            blocks,
            layout: crate::ocr::structured_output::PageLayout {
                width: 600.0,
                height: 1000.0,
                text_regions: vec![],
            },
        }
    }

    #[test]
    fn test_seller_name_extraction_regex() {
        let ocr = create_ocr_output(vec!["销售方：北京科技有限公司"]);
        let extractor = SellerNameExtractor::new();
        let result = extractor.extract(&ocr);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "北京科技有限公司");
    }

    #[test]
    fn test_seller_name_extraction_from_vat() {
        let ocr = create_ocr_output(vec![
            "销售方信息",
            "名称：四川景澜酒店管理有限公司",
            "统一社会信用代码：91510100MA6C",
        ]);
        let extractor = SellerNameExtractor::new();
        let result = extractor.extract(&ocr);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "四川景澜酒店管理有限公司");
    }

    #[test]
    fn test_amount_extraction_vat() {
        let ocr = create_ocr_output(vec!["价税合计：¥1045.24"]);
        let extractor = AmountExtractor::new();
        let result = extractor.extract(&ocr);
        assert!(result.is_some());
        let value: f64 = result.unwrap().value.replace(",", "").parse().unwrap();
        assert!((value - 1045.24).abs() < 0.01);
    }

    #[test]
    fn test_amount_extraction_max_fallback() {
        let ocr = create_ocr_output(vec!["消费 ￥30.00 其他 ￥200.00"]);
        let extractor = AmountExtractor::new();
        let result = extractor.extract_max_amount(&ocr);
        assert!(result.is_some());
        let value: f64 = result.unwrap().value.parse().unwrap();
        assert!((value - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_item_name_extraction() {
        let ocr = create_ocr_output(vec!["项目名称：交通服务费"]);
        let extractor = ItemNameExtractor::new();
        let result = extractor.extract(&ocr);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "交通服务费");
    }

    #[test]
    fn test_invoice_number_extraction() {
        let ocr = create_ocr_output(vec!["发票号码：12345678"]);
        let extractor = InvoiceNumberExtractor::new();
        let result = extractor.extract(&ocr);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "12345678");
    }

    #[test]
    fn test_date_extraction() {
        let ocr = create_ocr_output(vec!["2025年06月15日"]);
        let extractor = DateExtractor::new();
        let result = extractor.extract(&ocr);
        assert!(result.is_some());
        let field = result.unwrap();
        let date = extractor.parse_to_date(&field);
        assert!(date.is_some());
        assert_eq!(date.unwrap(), chrono::NaiveDate::from_ymd_opt(2025, 6, 15).unwrap());
    }

    #[test]
    fn test_regex_strategy_multiple_patterns() {
        let ocr = create_ocr_output(vec!["金额：100.00元"]);
        let strategy = RegexStrategy::new("金额", &[r"价格[：:]?\s*(.+)", r"金额[：:]?\s*(.+?)元"]);
        let result = strategy.extract(&ocr);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "100.00");
    }

    #[test]
    fn test_key_value_proximity_strategy() {
        let ocr = create_ocr_output(vec![
            "销售方信息",
            "名称：四川景澜酒店管理有限公司",
            "税号：123456789",
        ]);
        let strategy = KeyValueProximityStrategy::new("销售方信息", "名称");
        let result = strategy.extract(&ocr);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "四川景澜酒店管理有限公司");
    }

    #[test]
    fn test_extracted_field_confidence() {
        let ocr = create_ocr_output(vec!["名称：测试公司"]);
        let extractor = SellerNameExtractor::new();
        let result = extractor.extract(&ocr);
        assert!(result.is_some());
        assert!(result.unwrap().confidence > 0.0);
    }
}
