use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrTextBlock {
    pub text: String,
    pub confidence: f64,
    pub bbox: BoundingBox,
    pub line_index: usize,
    pub block_type: TextBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextBlockType {
    Title,
    KeyValue,
    Table,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrStructuredOutput {
    pub blocks: Vec<OcrTextBlock>,
    pub layout: PageLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageLayout {
    pub width: f64,
    pub height: f64,
    pub text_regions: Vec<TextRegion>,
}

impl Default for PageLayout {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            text_regions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegion {
    pub region_type: RegionType,
    pub bbox: BoundingBox,
    pub block_indices: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegionType {
    Header,
    Body,
    Table,
    Footer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_text_block_creation() {
        let block = OcrTextBlock {
            text: "测试文本".to_string(),
            confidence: 0.95,
            bbox: BoundingBox {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 30.0,
            },
            line_index: 0,
            block_type: TextBlockType::KeyValue,
        };

        assert_eq!(block.text, "测试文本");
        assert!((block.confidence - 0.95).abs() < 0.01);
        assert_eq!(block.bbox.x, 10.0);
        assert_eq!(block.bbox.y, 20.0);
        assert_eq!(block.line_index, 0);
        assert!(matches!(block.block_type, TextBlockType::KeyValue));
    }

    #[test]
    fn test_ocr_text_block_serialization() {
        let block = OcrTextBlock {
            text: "发票金额".to_string(),
            confidence: 0.99,
            bbox: BoundingBox {
                x: 0.0,
                y: 100.0,
                width: 150.0,
                height: 25.0,
            },
            line_index: 1,
            block_type: TextBlockType::Title,
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("发票金额"));
        assert!(json.contains("0.99"));

        let deserialized: OcrTextBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text, block.text);
        assert!((deserialized.confidence - block.confidence).abs() < 0.01);
    }

    #[test]
    fn test_bounding_box_default() {
        let bbox = BoundingBox::default();
        assert_eq!(bbox.x, 0.0);
        assert_eq!(bbox.y, 0.0);
        assert_eq!(bbox.width, 0.0);
        assert_eq!(bbox.height, 0.0);
    }

    #[test]
    fn test_bounding_box_coordinates() {
        let bbox = BoundingBox {
            x: 50.5,
            y: 100.25,
            width: 200.75,
            height: 50.0,
        };

        assert_eq!(bbox.x, 50.5);
        assert_eq!(bbox.y, 100.25);
        assert_eq!(bbox.width, 200.75);
        assert_eq!(bbox.height, 50.0);

        let right = bbox.x + bbox.width;
        let bottom = bbox.y + bbox.height;
        assert_eq!(right, 251.25);
        assert_eq!(bottom, 150.25);
    }

    #[test]
    fn test_text_block_type_serialization() {
        let types = vec![
            TextBlockType::Title,
            TextBlockType::KeyValue,
            TextBlockType::Table,
            TextBlockType::Other,
        ];

        for block_type in types {
            let json = serde_json::to_string(&block_type).unwrap();
            let deserialized: TextBlockType = serde_json::from_str(&json).unwrap();
            assert_eq!(std::mem::discriminant(&deserialized), std::mem::discriminant(&block_type));
        }
    }

    #[test]
    fn test_ocr_structured_output_construction() {
        let blocks = vec![
            OcrTextBlock {
                text: "增值税发票".to_string(),
                confidence: 0.98,
                bbox: BoundingBox {
                    x: 0.0,
                    y: 0.0,
                    width: 200.0,
                    height: 30.0,
                },
                line_index: 0,
                block_type: TextBlockType::Title,
            },
            OcrTextBlock {
                text: "金额：100.00".to_string(),
                confidence: 0.95,
                bbox: BoundingBox {
                    x: 0.0,
                    y: 50.0,
                    width: 150.0,
                    height: 20.0,
                },
                line_index: 1,
                block_type: TextBlockType::KeyValue,
            },
        ];

        let output = OcrStructuredOutput {
            blocks: blocks.clone(),
            layout: PageLayout::default(),
        };

        assert_eq!(output.blocks.len(), 2);
        assert_eq!(output.blocks[0].text, "增值税发票");
        assert_eq!(output.blocks[1].text, "金额：100.00");
        assert_eq!(output.layout.width, 0.0);
        assert_eq!(output.layout.height, 0.0);
        assert!(output.layout.text_regions.is_empty());
    }

    #[test]
    fn test_vat_invoice_ocr_output() {
        let ocr_blocks = vec![
            OcrTextBlock {
                text: "销售方信息".to_string(),
                confidence: 0.99,
                bbox: BoundingBox {
                    x: 0.0,
                    y: 100.0,
                    width: 100.0,
                    height: 20.0,
                },
                line_index: 0,
                block_type: TextBlockType::Title,
            },
            OcrTextBlock {
                text: "名称：四川景澜酒店管理有限公司".to_string(),
                confidence: 0.95,
                bbox: BoundingBox {
                    x: 0.0,
                    y: 120.0,
                    width: 200.0,
                    height: 20.0,
                },
                line_index: 1,
                block_type: TextBlockType::KeyValue,
            },
            OcrTextBlock {
                text: "价税合计：¥1045.24".to_string(),
                confidence: 0.97,
                bbox: BoundingBox {
                    x: 0.0,
                    y: 200.0,
                    width: 180.0,
                    height: 25.0,
                },
                line_index: 2,
                block_type: TextBlockType::KeyValue,
            },
        ];

        let output = OcrStructuredOutput {
            blocks: ocr_blocks,
            layout: PageLayout::default(),
        };

        assert_eq!(output.blocks.len(), 3);
        assert!(output.blocks.iter().any(|b| b.text.contains("销售方")));
        assert!(output.blocks.iter().any(|b| b.text.contains("四川景澜")));
        assert!(output.blocks.iter().any(|b| b.text.contains("1045.24")));
    }

    #[test]
    fn test_didi_trip_ocr_output() {
        let ocr_blocks = vec![
            OcrTextBlock {
                text: "滴滴出行行程单".to_string(),
                confidence: 0.98,
                bbox: BoundingBox {
                    x: 100.0,
                    y: 0.0,
                    width: 200.0,
                    height: 30.0,
                },
                line_index: 0,
                block_type: TextBlockType::Title,
            },
            OcrTextBlock {
                text: "出发时间：2024-01-15 09:30".to_string(),
                confidence: 0.92,
                bbox: BoundingBox {
                    x: 50.0,
                    y: 50.0,
                    width: 250.0,
                    height: 20.0,
                },
                line_index: 1,
                block_type: TextBlockType::KeyValue,
            },
            OcrTextBlock {
                text: "金额：35.50元".to_string(),
                confidence: 0.96,
                bbox: BoundingBox {
                    x: 50.0,
                    y: 100.0,
                    width: 150.0,
                    height: 20.0,
                },
                line_index: 2,
                block_type: TextBlockType::KeyValue,
            },
        ];

        let output = OcrStructuredOutput {
            blocks: ocr_blocks,
            layout: PageLayout {
                width: 600.0,
                height: 800.0,
                text_regions: vec![],
            },
        };

        assert_eq!(output.blocks.len(), 3);
        assert!(output.blocks.iter().any(|b| b.text.contains("滴滴")));
        assert!(output.blocks.iter().any(|b| b.text.contains("35.50")));
        assert_eq!(output.layout.width, 600.0);
        assert_eq!(output.layout.height, 800.0);
    }

    #[test]
    fn test_confidence_filtering() {
        let blocks = vec![
            OcrTextBlock {
                text: "高置信度文本".to_string(),
                confidence: 0.95,
                bbox: BoundingBox::default(),
                line_index: 0,
                block_type: TextBlockType::Other,
            },
            OcrTextBlock {
                text: "中等置信度".to_string(),
                confidence: 0.70,
                bbox: BoundingBox::default(),
                line_index: 1,
                block_type: TextBlockType::Other,
            },
            OcrTextBlock {
                text: "低置信度".to_string(),
                confidence: 0.50,
                bbox: BoundingBox::default(),
                line_index: 2,
                block_type: TextBlockType::Other,
            },
        ];

        let high_confidence: Vec<_> = blocks.iter().filter(|b| b.confidence > 0.9).collect();
        let medium_confidence: Vec<_> = blocks.iter().filter(|b| b.confidence > 0.6 && b.confidence <= 0.9).collect();
        let low_confidence: Vec<_> = blocks.iter().filter(|b| b.confidence <= 0.6).collect();

        assert_eq!(high_confidence.len(), 1);
        assert_eq!(medium_confidence.len(), 1);
        assert_eq!(low_confidence.len(), 1);
    }

    #[test]
    fn test_page_layout_with_regions() {
        let text_region = TextRegion {
            region_type: RegionType::Header,
            bbox: BoundingBox {
                x: 0.0,
                y: 0.0,
                width: 600.0,
                height: 100.0,
            },
            block_indices: vec![0, 1],
        };

        let layout = PageLayout {
            width: 600.0,
            height: 800.0,
            text_regions: vec![text_region],
        };

        assert_eq!(layout.width, 600.0);
        assert_eq!(layout.height, 800.0);
        assert_eq!(layout.text_regions.len(), 1);
        assert_eq!(layout.text_regions[0].block_indices.len(), 2);
    }

    #[test]
    fn test_region_type_variants() {
        let types = vec![
            RegionType::Header,
            RegionType::Body,
            RegionType::Table,
            RegionType::Footer,
        ];

        for region_type in types {
            let json = serde_json::to_string(&region_type).unwrap();
            let deserialized: RegionType = serde_json::from_str(&json).unwrap();
            assert_eq!(std::mem::discriminant(&deserialized), std::mem::discriminant(&region_type));
        }
    }
}
