use serde::{Deserialize, Serialize};
use paddle_ocr_rs::ocr_lite::OcrLite;
use std::path::Path;

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

pub struct OcrEngine {
    ocr: OcrLite,
}

impl OcrEngine {
    /// 创建新的 OCR 引擎并加载模型
    /// models_dir 应包含:
    ///   - ch_PP-OCRv5_mobile_det.onnx (检测模型)
    ///   - ch_ppocr_mobile_v2.0_cls_infer.onnx (分类模型)
    ///   - ch_PP-OCRv5_rec_mobile_infer.onnx (识别模型)
    ///   - ppocr_keys_v1.txt (字典文件，可选，v5 rec 内置)
    pub fn new(models_dir: &str) -> Result<Self, String> {
        let det_model = Path::new(models_dir).join("ch_PP-OCRv5_mobile_det.onnx");
        let cls_model = Path::new(models_dir).join("ch_ppocr_mobile_v2.0_cls_infer.onnx");
        let rec_model = Path::new(models_dir).join("ch_PP-OCRv5_rec_mobile_infer.onnx");
        let dict_file = Path::new(models_dir).join("ppocr_keys_v1.txt");

        for (name, path) in [("det", &det_model), ("cls", &cls_model), ("rec", &rec_model), ("dict", &dict_file)] {
            if !path.exists() {
                return Err(format!(
                    "OCR model file not found: {} ({})",
                    name,
                    path.display()
                ));
            }
        }

        let mut ocr = OcrLite::new();
        ocr.init_models_with_dict(
            det_model.to_str().unwrap(),
            cls_model.to_str().unwrap(),
            rec_model.to_str().unwrap(),
            dict_file.to_str().unwrap(),
            2, // num_thread
        )
        .map_err(|e| format!("Failed to init PaddleOCR: {:?}", e))?;

        Ok(Self { ocr })
    }

    /// 健康检查 - 嵌入式引擎始终可用
    pub fn health(&self) -> Result<bool, String> {
        Ok(true)
    }

    /// 识别图片中的文字
    pub fn recognize_image(&mut self, file_path: &str) -> Result<OcrImageResponse, String> {
        let result = self
            .ocr
            .detect_from_path(
                file_path,
                50,    // padding
                1024,  // max_side_len
                0.5,   // box_score_thresh
                0.6,   // box_thresh (v5: 0.6)
                1.5,   // un_clip_ratio (v5: 1.5)
                true,  // do_angle
                false, // most_angle
            )
            .map_err(|e| format!("OCR recognition failed: {:?}", e))?;

        let texts = result
            .text_blocks
            .iter()
            .map(|item| {
                let box_coords = Some(serde_json::json!({
                    "points": item.box_points.iter().map(|p| {
                        serde_json::json!({"x": p.x, "y": p.y})
                    }).collect::<Vec<_>>(),
                    "box_score": item.box_score,
                    "angle_index": item.angle_index,
                    "angle_score": item.angle_score,
                }));
                OcrTextItem {
                    text: item.text.clone(),
                    confidence: item.text_score as f64,
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
        let result = self
            .ocr
            .detect(
                img,
                50,    // padding
                1024,  // max_side_len
                0.5,   // box_score_thresh
                0.6,   // box_thresh (v5: 0.6)
                1.5,   // un_clip_ratio (v5: 1.5)
                true,  // do_angle
                false, // most_angle
            )
            .map_err(|e| format!("OCR recognition failed: {:?}", e))?;

        let texts = result
            .text_blocks
            .iter()
            .map(|item| {
                let box_coords = Some(serde_json::json!({
                    "points": item.box_points.iter().map(|p| {
                        serde_json::json!({"x": p.x, "y": p.y})
                    }).collect::<Vec<_>>(),
                    "box_score": item.box_score,
                    "angle_index": item.angle_index,
                    "angle_score": item.angle_score,
                }));
                OcrTextItem {
                    text: item.text.clone(),
                    confidence: item.text_score as f64,
                    box_coords,
                }
            })
            .collect();

        Ok(OcrImageResponse { texts })
    }

    /// 识别 PDF 中的文字
    /// 当前嵌入式模式暂不支持 PDF 直接识别，需要先转换为图片
    pub fn recognize_pdf(&mut self, file_path: &str) -> Result<OcrPdfResponse, String> {
        // 尝试用 pdf-render 将第一页渲染为图片
        // 当前简化实现：提示用户转换格式
        Err(format!(
            "PDF OCR not supported in embedded mode yet. Please convert '{}' to image first (PNG/JPG).",
            file_path
        ))
    }
}
