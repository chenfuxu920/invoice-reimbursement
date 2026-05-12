use invoice_reimbursement_lib::ocr::OcrEngine;
use std::path::Path;

const MODELS_DIR: &str = "models";

fn main() {
    println!("Initializing OCR engine...");
    let mut engine = OcrEngine::new(MODELS_DIR).expect("Failed to init OCR");
    println!("OCR engine initialized successfully!");
    
    // Create a simple test image using image crate
    let img = create_test_image();
    let test_path = "/tmp/test_ocr.png";
    img.save(test_path).expect("Failed to save test image");
    
    println!("\nTesting OCR on simple text image...");
    match engine.recognize_image(test_path) {
        Ok(result) => {
            println!("OCR Result:");
            for (i, text) in result.texts.iter().enumerate() {
                println!("  [{}] '{}' (conf={:.2})", i, text.text, text.confidence);
            }
            
            if result.texts.is_empty() {
                println!("  No text detected!");
            }
        }
        Err(e) => println!("OCR failed: {}", e),
    }
}

fn create_test_image() -> image::DynamicImage {
    use image::{DynamicImage, ImageBuffer, Rgb};
    
    // Create white background
    let mut img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(800, 200);
    
    // Fill with white
    for pixel in img.pixels_mut() {
        *pixel = Rgb([255, 255, 255]);
    }
    
    // Note: We can't easily draw text without external crates,
    // so this is just a blank image test
    DynamicImage::ImageRgb8(img)
}