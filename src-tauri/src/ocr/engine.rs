use serde::{Deserialize, Serialize};
use ocr_rs::OcrEngine as PaddleOcrEngine;
use std::path::Path;
use super::structured_output::{OcrStructuredOutput, OcrTextBlock, BoundingBox, TextBlockType, PageLayout, TextRegion, RegionType};

// 保留原有数据结构（被 parser 和前端使用）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcrTextItem {
    pub text: String,
    pub confidence: f64,
    pub box_coords: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcrImageResponse {
    pub texts: Vec<OcrTextItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcrPageResult {
    pub page: u32,
    pub texts: Vec<OcrTextItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OcrPdfResponse {
    pub pages: Vec<OcrPageResult>,
}

/// Construct box_coords JSON from bounding box coordinates and score.
/// Format: {points: [{x,y}*4], box_score: f64} — shared by OCR and pdfplumber extraction.
pub fn bbox_to_json(x0: f64, y0: f64, x1: f64, y1: f64, score: f64) -> serde_json::Value {
    serde_json::json!({
        "points": [
            {"x": x0, "y": y0},
            {"x": x1, "y": y0},
            {"x": x1, "y": y1},
            {"x": x0, "y": y1}
        ],
        "box_score": score,
    })
}

pub struct OcrEngine {
    engine: Option<PaddleOcrEngine>,
}

impl OcrEngine {
    /// 创建未初始化的引擎（模型未安装，recognize_* 将返回错误，文本提取不受影响）
    pub fn uninitialized() -> Self {
        Self { engine: None }
    }

    /// 创建新的 OCR 引擎并加载模型
    /// models_dir 应包含:
    ///   - PP-OCRv5_mobile_det.mnn (检测模型)
    ///   - PP-OCRv5_mobile_rec.mnn (识别模型)
    ///   - ppocr_keys_v5.txt (字典文件)
    /// 模型文件不存在时返回未初始化引擎（不报错，OCR 不可用但应用正常运行）
    pub fn new(models_dir: &str) -> Result<Self, String> {
        let det_model = Path::new(models_dir).join("PP-OCRv5_mobile_det.mnn");
        let rec_model = Path::new(models_dir).join("PP-OCRv5_mobile_rec.mnn");
        let dict_file = Path::new(models_dir).join("ppocr_keys_v5.txt");

        for (name, path) in [("det", &det_model), ("rec", &rec_model), ("dict", &dict_file)] {
            if !path.exists() {
                eprintln!("OCR model file not found: {} ({})", name, path.display());
                return Ok(Self::uninitialized());
            }
        }

        let engine = PaddleOcrEngine::new(
            det_model.to_str().unwrap(),
            rec_model.to_str().unwrap(),
            dict_file.to_str().unwrap(),
            None,
        ).map_err(|e| format!("Failed to init PaddleOCR: {:?}", e))?;

        Ok(Self { engine: Some(engine) })
    }

    /// 健康检查 - 返回 OCR 引擎是否可用（模型是否已加载）
    pub fn health(&self) -> Result<bool, String> {
        Ok(self.engine.is_some())
    }

    /// 识别图片中的文字
    pub fn recognize_image(&mut self, file_path: &str) -> Result<OcrImageResponse, String> {
        let engine = self.engine.as_mut()
            .ok_or("OCR 模型未安装，请先在首页下载 OCR 模型")?;

        let image = image::open(file_path)
            .map_err(|e| format!("Failed to open image: {:?}", e))?;
        
        let results = engine.recognize(&image)
            .map_err(|e| format!("OCR recognition failed: {:?}", e))?;

        let texts = results
            .iter()
            .map(|item| {
                let bbox = &item.bbox;
                let rect = bbox.rect;
                let box_coords = Some(bbox_to_json(
                    rect.left() as f64,
                    rect.top() as f64,
                    rect.right() as f64,
                    rect.bottom() as f64,
                    bbox.score as f64,
                ));
                OcrTextItem {
                    text: item.text.clone(),
                    confidence: item.confidence as f64,
                    box_coords,
                }
            })
            .collect();

        Ok(OcrImageResponse { texts })
    }

    /// 识别内存中图片的文字（用于 PDF 逐页处理）
    pub fn recognize_rgb_image(
        &mut self,
        img: &image::RgbImage,
    ) -> Result<OcrImageResponse, String> {
        let engine = self.engine.as_mut()
            .ok_or("OCR 模型未安装，请先在首页下载 OCR 模型")?;

        let dynamic_image = image::DynamicImage::ImageRgb8(img.clone());
        let results = engine.recognize(&dynamic_image)
            .map_err(|e| format!("OCR recognition failed: {:?}", e))?;

        let texts = results
            .iter()
            .map(|item| {
                let bbox = &item.bbox;
                let rect = bbox.rect;
                let box_coords = Some(bbox_to_json(
                    rect.left() as f64,
                    rect.top() as f64,
                    rect.right() as f64,
                    rect.bottom() as f64,
                    bbox.score as f64,
                ));
                OcrTextItem {
                    text: item.text.clone(),
                    confidence: item.confidence as f64,
                    box_coords,
                }
            })
            .collect();

        Ok(OcrImageResponse { texts })
    }

    /// 识别 PDF 中的文字
    /// 先尝试文字提取，失败则将 PDF 转图片后逐页 OCR
    pub fn recognize_pdf(&mut self, file_path: &str) -> Result<OcrPdfResponse, String> {
        // 将 PDF 渲染为图片再 OCR（纯 Rust zpdf 引擎）
        match render_pdf_to_images(file_path) {
            Ok(images) if !images.is_empty() => {
                let mut all_pages = Vec::new();
                for img in images {
                    let resp = self.recognize_rgb_image(&img)?;
                    all_pages.push(OcrPageResult {
                        page: all_pages.len() as u32 + 1,
                        texts: resp.texts,
                    });
                }
                Ok(OcrPdfResponse { pages: all_pages })
            }
            _ => {
                match pdftoppm_to_images(file_path) {
                    Ok(images) if !images.is_empty() => {
                        let mut all_pages = Vec::new();
                        for img_path in images {
                            let resp = self.recognize_image(&img_path)?;
                            all_pages.push(OcrPageResult {
                                page: all_pages.len() as u32 + 1,
                                texts: resp.texts,
                            });
                            std::fs::remove_file(&img_path).ok();
                        }
                        Ok(OcrPdfResponse { pages: all_pages })
                    }
                    _ => Err(format!(
                        "PDF OCR 失败: 无法解析 '{}'。请确保已安装 poppler-utils (pdftoppm)，或将 PDF 转为图片后导入。",
                        file_path
                    )),
                }
            }
        }
    }

    /// 将原始 OCR 结果转换为结构化输出
    pub fn process_to_structured(
        &mut self,
        file_path: &str,
    ) -> Result<OcrStructuredOutput, String> {
        let raw_response = self.recognize_image(file_path)?;

        let filtered: Vec<OcrTextItem> = raw_response
            .texts
            .into_iter()
            .filter(|t| t.confidence > 0.6)
            .collect();

        let mut blocks: Vec<OcrTextBlock> = Vec::new();
        for (idx, item) in filtered.iter().enumerate() {
            let bbox = parse_box_coords(&item.box_coords)?;
            blocks.push(OcrTextBlock {
                text: item.text.clone(),
                confidence: item.confidence,
                bbox,
                line_index: idx,
                block_type: infer_block_type(&item.text),
            });
        }

        let regions = cluster_text_regions(&blocks);

        Ok(OcrStructuredOutput {
            blocks,
            layout: PageLayout {
                width: 0.0,
                height: 0.0,
                text_regions: regions,
            },
        })
    }
}

/// 解析边界框坐标
fn parse_box_coords(box_coords: &Option<serde_json::Value>) -> Result<BoundingBox, String> {
    match box_coords {
        Some(coords) => {
            if let Some(points) = coords.get("points").and_then(|p| p.as_array()) {
                if points.len() >= 4 {
                    let x_values: Vec<f64> = points
                        .iter()
                        .filter_map(|p| p.get("x").and_then(|x| x.as_f64()))
                        .collect();
                    let y_values: Vec<f64> = points
                        .iter()
                        .filter_map(|p| p.get("y").and_then(|y| y.as_f64()))
                        .collect();

                    if !x_values.is_empty() && !y_values.is_empty() {
                        let min_x = x_values.iter().cloned().fold(f64::INFINITY, f64::min);
                        let max_x = x_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                        let min_y = y_values.iter().cloned().fold(f64::INFINITY, f64::min);
                        let max_y = y_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

                        return Ok(BoundingBox {
                            x: min_x,
                            y: min_y,
                            width: max_x - min_x,
                            height: max_y - min_y,
                        });
                    }
                }
            }
            Ok(BoundingBox::default())
        }
        None => Ok(BoundingBox::default()),
    }
}

/// 推断文本块类型
fn infer_block_type(text: &str) -> TextBlockType {
    if text.contains("发票") || text.contains("凭证") {
        TextBlockType::Title
    } else if text.contains("：") || text.contains(":") {
        TextBlockType::KeyValue
    } else {
        TextBlockType::Other
    }
}

/// 基于空间位置聚类文本区域
fn cluster_text_regions(blocks: &[OcrTextBlock]) -> Vec<TextRegion> {
    if blocks.is_empty() {
        return vec![];
    }

    let max_y = blocks.iter().map(|b| b.bbox.y + b.bbox.height).fold(0.0, f64::max);
    if max_y == 0.0 {
        return vec![];
    }

    let header_threshold = max_y * 0.2;
    let footer_threshold = max_y * 0.8;

    let mut header_indices = Vec::new();
    let mut body_indices = Vec::new();
    let mut footer_indices = Vec::new();

    for (idx, block) in blocks.iter().enumerate() {
        let block_center_y = block.bbox.y + block.bbox.height / 2.0;

        if block_center_y < header_threshold {
            header_indices.push(idx);
        } else if block_center_y > footer_threshold {
            footer_indices.push(idx);
        } else {
            body_indices.push(idx);
        }
    }

    let mut regions = Vec::new();

    if !header_indices.is_empty() {
        let bbox = calculate_region_bbox(blocks, &header_indices);
        regions.push(TextRegion {
            region_type: RegionType::Header,
            bbox,
            block_indices: header_indices,
        });
    }

    if !body_indices.is_empty() {
        let bbox = calculate_region_bbox(blocks, &body_indices);
        regions.push(TextRegion {
            region_type: RegionType::Body,
            bbox,
            block_indices: body_indices,
        });
    }

    if !footer_indices.is_empty() {
        let bbox = calculate_region_bbox(blocks, &footer_indices);
        regions.push(TextRegion {
            region_type: RegionType::Footer,
            bbox,
            block_indices: footer_indices,
        });
    }

    regions
}

/// 计算文本区域的边界框
fn calculate_region_bbox(blocks: &[OcrTextBlock], indices: &[usize]) -> BoundingBox {
    if indices.is_empty() {
        return BoundingBox::default();
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for &idx in indices {
        if let Some(block) = blocks.get(idx) {
            min_x = min_x.min(block.bbox.x);
            min_y = min_y.min(block.bbox.y);
            max_x = max_x.max(block.bbox.x + block.bbox.width);
            max_y = max_y.max(block.bbox.y + block.bbox.height);
        }
    }

    BoundingBox {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

/// 使用 zpdf（纯 Rust）将 PDF 渲染为图片，无需外部 DLL
fn render_pdf_to_images(pdf_path: &str) -> Result<Vec<image::RgbImage>, String> {
    crate::pdf::image_embedder::render_pdf_to_rgb_images(pdf_path, 200)
}

/// 查找 pdftoppm 可执行文件路径（在 PATH 或常见安装目录中搜索）
fn find_pdftoppm() -> Option<std::path::PathBuf> {
    // 1. 尝试 PATH
    if let Ok(path) = std::process::Command::new("pdftoppm").arg("--version").output() {
        if path.status.success() {
            // 在 PATH 中能找到，返回默认名称即可（Command::new 会搜索 PATH）
            // 但我们无法直接获取 PATH 中的完整路径，返回 None 让调用方使用 "pdftoppm"
            return None;
        }
    }

    // 2. 在常见的 Windows 安装路径中搜索
    let common_paths = [
        // WinGet
        r"C:\Program Files\Poppler\poppler\Library\bin\pdftoppm.exe",
        // WinGet (user)
        r"C:\Users\chenf\AppData\Local\Microsoft\WinGet\Packages\oschwartz10612.Poppler_Microsoft.Winget.Source_8wekyb3d8bbwe\poppler-25.07.0\Library\bin\pdftoppm.exe",
        // Chocolatey
        r"C:\ProgramData\chocolatey\lib\poppler\tools\poppler\Library\bin\pdftoppm.exe",
        // msys2
        r"C:\msys64\mingw64\bin\pdftoppm.exe",
    ];

    for path in &common_paths {
        let p = std::path::Path::new(path);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }

    None
}

/// 使用 pdftoppm 命令行工具将 PDF 转为 PNG 图片
fn pdftoppm_to_images(pdf_path: &str) -> Result<Vec<String>, String> {
    use std::process::Command;

    let output_dir = std::env::temp_dir().join("invoice_ocr");
    std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;

    let pdf_name = std::path::Path::new(pdf_path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();

    let output_prefix = output_dir.join(pdf_name.as_ref());

    let mut pdftoppm_cmd: std::process::Command = match find_pdftoppm() {
        Some(p) => Command::new(&p),
        None => Command::new("pdftoppm"),
    };

    let output = pdftoppm_cmd
        .args([
            "-png",
            "-f", "1",
            "-l", "5",  // 最多处理 5 页
            "-r", "200",
            pdf_path,
            output_prefix.to_str().unwrap_or_default(),
        ])
        .output()
        .map_err(|_| "pdftoppm not found. Install poppler-utils.".to_string())?;

    if !output.status.success() {
        return Err(format!(
            "pdftoppm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // 收集生成的 PNG 文件
    let mut images = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "png") {
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with(pdf_name.as_ref()) {
                        images.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    images.sort();
    Ok(images)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_box_coords_with_valid_points() {
        let json_str = r#"{
            "points": [
                {"x": 10.0, "y": 20.0},
                {"x": 110.0, "y": 20.0},
                {"x": 110.0, "y": 50.0},
                {"x": 10.0, "y": 50.0}
            ]
        }"#;
        let box_coords: Option<serde_json::Value> = Some(serde_json::from_str(json_str).unwrap());

        let bbox = parse_box_coords(&box_coords).unwrap();

        assert_eq!(bbox.x, 10.0);
        assert_eq!(bbox.y, 20.0);
        assert_eq!(bbox.width, 100.0);
        assert_eq!(bbox.height, 30.0);
    }

    #[test]
    fn test_parse_box_coords_with_none() {
        let bbox = parse_box_coords(&None).unwrap();
        assert_eq!(bbox.x, 0.0);
        assert_eq!(bbox.y, 0.0);
        assert_eq!(bbox.width, 0.0);
        assert_eq!(bbox.height, 0.0);
    }

    #[test]
    fn test_parse_box_coords_with_empty_points() {
        let json_str = r#"{"points": []}"#;
        let box_coords: Option<serde_json::Value> = Some(serde_json::from_str(json_str).unwrap());

        let bbox = parse_box_coords(&box_coords).unwrap();
        assert_eq!(bbox.x, 0.0);
        assert_eq!(bbox.y, 0.0);
    }

    #[test]
    fn test_parse_box_coords_with_partial_points() {
        let json_str = r#"{
            "points": [
                {"x": 50.0, "y": 100.0},
                {"x": 150.0, "y": 100.0}
            ]
        }"#;
        let box_coords: Option<serde_json::Value> = Some(serde_json::from_str(json_str).unwrap());

        let bbox = parse_box_coords(&box_coords).unwrap();
        assert_eq!(bbox.x, 0.0);
        assert_eq!(bbox.y, 0.0);
        assert_eq!(bbox.width, 0.0);
        assert_eq!(bbox.height, 0.0);
    }

    #[test]
    fn test_parse_box_coords_without_points() {
        let json_str = r#"{"other_field": "value"}"#;
        let box_coords: Option<serde_json::Value> = Some(serde_json::from_str(json_str).unwrap());

        let bbox = parse_box_coords(&box_coords).unwrap();
        assert_eq!(bbox.x, 0.0);
        assert_eq!(bbox.y, 0.0);
    }

    #[test]
    fn test_infer_block_type_title() {
        assert!(matches!(
            infer_block_type("增值税专用发票"),
            TextBlockType::Title
        ));
        assert!(matches!(infer_block_type("发票"), TextBlockType::Title));
        assert!(matches!(
            infer_block_type("电子凭证"),
            TextBlockType::Title
        ));
    }

    #[test]
    fn test_infer_block_type_key_value() {
        assert!(matches!(
            infer_block_type("名称：测试公司"),
            TextBlockType::KeyValue
        ));
        assert!(matches!(
            infer_block_type("金额:100元"),
            TextBlockType::KeyValue
        ));
        assert!(matches!(
            infer_block_type("日期：2024-01-01"),
            TextBlockType::KeyValue
        ));
    }

    #[test]
    fn test_infer_block_type_other() {
        assert!(matches!(infer_block_type("一些普通文本"), TextBlockType::Other));
        assert!(matches!(
            infer_block_type("四川景澜酒店"),
            TextBlockType::Other
        ));
        assert!(matches!(infer_block_type("12345"), TextBlockType::Other));
    }

    #[test]
    fn test_cluster_text_regions_empty_blocks() {
        let blocks = vec![];
        let regions = cluster_text_regions(&blocks);
        assert!(regions.is_empty());
    }

    #[test]
    fn test_cluster_text_regions_single_block() {
        let blocks = vec![OcrTextBlock {
            text: "标题".to_string(),
            confidence: 0.99,
            bbox: BoundingBox {
                x: 0.0,
                y: 10.0,
                width: 100.0,
                height: 20.0,
            },
            line_index: 0,
            block_type: TextBlockType::Title,
        }];

        let regions = cluster_text_regions(&blocks);

        assert!(!regions.is_empty());

        let total_blocks: usize = regions.iter().map(|r| r.block_indices.len()).sum();
        assert_eq!(total_blocks, 1);
    }

    #[test]
    fn test_cluster_text_regions_multiple_blocks() {
        let blocks = vec![
            OcrTextBlock {
                text: "页眉".to_string(),
                confidence: 0.99,
                bbox: BoundingBox {
                    x: 0.0,
                    y: 10.0,
                    width: 100.0,
                    height: 20.0,
                },
                line_index: 0,
                block_type: TextBlockType::Title,
            },
            OcrTextBlock {
                text: "正文内容".to_string(),
                confidence: 0.95,
                bbox: BoundingBox {
                    x: 0.0,
                    y: 150.0,
                    width: 200.0,
                    height: 30.0,
                },
                line_index: 1,
                block_type: TextBlockType::Other,
            },
            OcrTextBlock {
                text: "页脚".to_string(),
                confidence: 0.92,
                bbox: BoundingBox {
                    x: 0.0,
                    y: 700.0,
                    width: 80.0,
                    height: 15.0,
                },
                line_index: 2,
                block_type: TextBlockType::Other,
            },
        ];

        let regions = cluster_text_regions(&blocks);

        assert!(!regions.is_empty());
        assert!(regions.iter().any(|r| matches!(r.region_type, RegionType::Header)));
        assert!(regions.iter().any(|r| matches!(r.region_type, RegionType::Body)));
    }

    #[test]
    fn test_calculate_region_bbox_empty_indices() {
        let blocks = vec![OcrTextBlock {
            text: "test".to_string(),
            confidence: 0.9,
            bbox: BoundingBox {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 30.0,
            },
            line_index: 0,
            block_type: TextBlockType::Other,
        }];

        let bbox = calculate_region_bbox(&blocks, &[]);

        assert_eq!(bbox.x, 0.0);
        assert_eq!(bbox.y, 0.0);
        assert_eq!(bbox.width, 0.0);
        assert_eq!(bbox.height, 0.0);
    }

    #[test]
    fn test_calculate_region_bbox_single_index() {
        let blocks = vec![OcrTextBlock {
            text: "test".to_string(),
            confidence: 0.9,
            bbox: BoundingBox {
                x: 50.0,
                y: 100.0,
                width: 150.0,
                height: 40.0,
            },
            line_index: 0,
            block_type: TextBlockType::Other,
        }];

        let bbox = calculate_region_bbox(&blocks, &[0]);

        assert_eq!(bbox.x, 50.0);
        assert_eq!(bbox.y, 100.0);
        assert_eq!(bbox.width, 150.0);
        assert_eq!(bbox.height, 40.0);
    }

    #[test]
    fn test_calculate_region_bbox_multiple_indices() {
        let blocks = vec![
            OcrTextBlock {
                text: "block1".to_string(),
                confidence: 0.9,
                bbox: BoundingBox {
                    x: 10.0,
                    y: 20.0,
                    width: 100.0,
                    height: 30.0,
                },
                line_index: 0,
                block_type: TextBlockType::Other,
            },
            OcrTextBlock {
                text: "block2".to_string(),
                confidence: 0.85,
                bbox: BoundingBox {
                    x: 150.0,
                    y: 80.0,
                    width: 120.0,
                    height: 25.0,
                },
                line_index: 1,
                block_type: TextBlockType::Other,
            },
        ];

        let bbox = calculate_region_bbox(&blocks, &[0, 1]);

        assert_eq!(bbox.x, 10.0);
        assert_eq!(bbox.y, 20.0);
        assert_eq!(bbox.width, 260.0);
        assert_eq!(bbox.height, 85.0);
    }

    #[test]
    fn test_confidence_filter_threshold() {
        let items = vec![
            OcrTextItem {
                text: "高置信度".to_string(),
                confidence: 0.95,
                box_coords: None,
            },
            OcrTextItem {
                text: "中等置信度".to_string(),
                confidence: 0.65,
                box_coords: None,
            },
            OcrTextItem {
                text: "低置信度".to_string(),
                confidence: 0.55,
                box_coords: None,
            },
        ];

        let filtered: Vec<_> = items.into_iter().filter(|t| t.confidence > 0.6).collect();

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|t| t.text == "高置信度"));
        assert!(filtered.iter().any(|t| t.text == "中等置信度"));
    }

    #[test]
    fn test_vat_invoice_parsing_scenario() {
        let box_coords = r#"{
            "points": [
                {"x": 0.0, "y": 100.0},
                {"x": 200.0, "y": 100.0},
                {"x": 200.0, "y": 120.0},
                {"x": 0.0, "y": 120.0}
            ]
        }"#;

        let item = OcrTextItem {
            text: "名称：四川景澜酒店管理有限公司".to_string(),
            confidence: 0.95,
            box_coords: Some(serde_json::from_str(box_coords).unwrap()),
        };

        let bbox = parse_box_coords(&item.box_coords).unwrap();
        let block_type = infer_block_type(&item.text);

        assert_eq!(bbox.x, 0.0);
        assert_eq!(bbox.y, 100.0);
        assert_eq!(bbox.width, 200.0);
        assert_eq!(bbox.height, 20.0);
        assert!(matches!(block_type, TextBlockType::KeyValue));
    }

    #[test]
    fn test_didi_invoice_parsing_scenario() {
        let box_coords = r#"{
            "points": [
                {"x": 50.0, "y": 200.0},
                {"x": 250.0, "y": 200.0},
                {"x": 250.0, "y": 220.0},
                {"x": 50.0, "y": 220.0}
            ]
        }"#;

        let item = OcrTextItem {
            text: "出发时间：2024-01-15 09:30".to_string(),
            confidence: 0.92,
            box_coords: Some(serde_json::from_str(box_coords).unwrap()),
        };

        let bbox = parse_box_coords(&item.box_coords).unwrap();
        let block_type = infer_block_type(&item.text);

        assert_eq!(bbox.x, 50.0);
        assert_eq!(bbox.y, 200.0);
        assert_eq!(bbox.width, 200.0);
        assert_eq!(bbox.height, 20.0);
        assert!(matches!(block_type, TextBlockType::KeyValue));
    }

    #[test]
    fn test_various_confidence_levels() {
        let test_cases = vec![
            (0.99, "极高置信度"),
            (0.95, "高置信度"),
            (0.85, "中高置信度"),
            (0.75, "中等置信度"),
            (0.65, "及格置信度"),
            (0.55, "不及格置信度"),
        ];

        for (confidence, label) in test_cases {
            let item = OcrTextItem {
                text: label.to_string(),
                confidence,
                box_coords: None,
            };

            assert!((item.confidence - confidence).abs() < 0.01);
        }
    }

    #[test]
    fn test_bounding_box_coordinate_formats() {
        let test_cases = vec![
            (r#"{"points": [{"x": 0.0, "y": 0.0}, {"x": 100.0, "y": 0.0}, {"x": 100.0, "y": 50.0}, {"x": 0.0, "y": 50.0}]}"#, 0.0, 0.0, 100.0, 50.0),
            (r#"{"points": [{"x": 50.5, "y": 25.3}, {"x": 150.7, "y": 25.3}, {"x": 150.7, "y": 75.8}, {"x": 50.5, "y": 75.8}]}"#, 50.5, 25.3, 100.2, 50.5),
            (r#"{"points": [{"x": 1000.0, "y": 500.0}, {"x": 1200.0, "y": 500.0}, {"x": 1200.0, "y": 550.0}, {"x": 1000.0, "y": 550.0}]}"#, 1000.0, 500.0, 200.0, 50.0),
        ];

        for (json_str, expected_x, expected_y, expected_w, expected_h) in test_cases {
            let box_coords: Option<serde_json::Value> = Some(serde_json::from_str(json_str).unwrap());
            let bbox = parse_box_coords(&box_coords).unwrap();

            assert!((bbox.x - expected_x).abs() < 0.1);
            assert!((bbox.y - expected_y).abs() < 0.1);
            assert!((bbox.width - expected_w).abs() < 0.1);
            assert!((bbox.height - expected_h).abs() < 0.1);
        }
    }

    #[test]
    fn test_cluster_regions_with_table_blocks() {
        let blocks = vec![
            OcrTextBlock {
                text: "商品名称".to_string(),
                confidence: 0.98,
                bbox: BoundingBox {
                    x: 0.0,
                    y: 100.0,
                    width: 100.0,
                    height: 20.0,
                },
                line_index: 0,
                block_type: TextBlockType::Table,
            },
            OcrTextBlock {
                text: "数量".to_string(),
                confidence: 0.97,
                bbox: BoundingBox {
                    x: 110.0,
                    y: 100.0,
                    width: 80.0,
                    height: 20.0,
                },
                line_index: 1,
                block_type: TextBlockType::Table,
            },
            OcrTextBlock {
                text: "单价".to_string(),
                confidence: 0.96,
                bbox: BoundingBox {
                    x: 200.0,
                    y: 100.0,
                    width: 80.0,
                    height: 20.0,
                },
                line_index: 2,
                block_type: TextBlockType::Table,
            },
        ];

        let regions = cluster_text_regions(&blocks);

        assert!(!regions.is_empty());

        let total_blocks: usize = regions.iter().map(|r| r.block_indices.len()).sum();
        assert_eq!(total_blocks, 3);
    }
}
