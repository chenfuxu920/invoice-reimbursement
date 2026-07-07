pub mod engine;
pub mod model_downloader;
pub mod structured_output;

pub use engine::{OcrEngine, OcrTextItem, OcrImageResponse, OcrPageResult, OcrPdfResponse, bbox_to_json};
pub use model_downloader::OcrModelConfig;
pub use structured_output::{
    OcrStructuredOutput, OcrTextBlock, BoundingBox, TextBlockType,
    PageLayout, TextRegion, RegionType,
};
