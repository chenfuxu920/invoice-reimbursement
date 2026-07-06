pub mod engine;
pub mod structured_output;

pub use engine::{OcrEngine, OcrTextItem, OcrImageResponse, OcrPageResult, OcrPdfResponse, bbox_to_json};
pub use structured_output::{
    OcrStructuredOutput, OcrTextBlock, BoundingBox, TextBlockType,
    PageLayout, TextRegion, RegionType,
};
