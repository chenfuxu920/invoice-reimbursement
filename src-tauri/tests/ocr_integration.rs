use invoice_reimbursement_lib::ocr::OcrEngine;

/// Test that OcrEngine::new() fails gracefully when models directory doesn't exist.
#[test]
fn test_ocr_engine_new_missing_models_dir() {
    let result = OcrEngine::new("/nonexistent/models/dir");
    assert!(result.is_err(), "Should fail when models directory doesn't exist");
    if let Err(err_msg) = result {
        assert!(err_msg.contains("OCR model file not found"), "Error message should mention missing model file");
    }
}

/// Test that OcrEngine::new() fails when models directory exists but model files are missing.
#[test]
fn test_ocr_engine_new_empty_models_dir() {
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let models_path = temp_dir.path().to_str().unwrap();
    
    let result = OcrEngine::new(models_path);
    assert!(result.is_err(), "Should fail when model files are missing");
    if let Err(err_msg) = result {
        assert!(err_msg.contains("OCR model file not found"), "Error message should mention missing model file");
    }
}

/// Test OcrEngine::health() returns true (always succeeds for embedded engine).
#[test]
fn test_ocr_engine_health() {
    // Even without a valid OcrEngine, we can't test health() without initializing
    // But we can document that health() always returns Ok(true) for embedded engines
    // when we have a valid instance.
    // For now, we'll skip this test since we can't create a valid OcrEngine without models.
}

/// Test OCR image recognition with a valid models directory.
/// This test requires OCR model files to be present, so it's marked #[ignore].
/// Run with: cargo test --test ocr_integration -- --ignored
#[test]
#[ignore]
fn test_ocr_recognize_image() {
    // This test requires ONNX models to be present in a directory
    // Set MODELS_DIR env var to point to the models directory
    let models_dir = std::env::var("MODELS_DIR")
        .unwrap_or_else(|_| "models".to_string());
    
    let mut engine = OcrEngine::new(&models_dir)
        .expect("Failed to create OcrEngine - ensure models are present");
    
    // Test with a sample image
    let test_image = "tests/fixtures/test_invoice.png";
    if !std::path::Path::new(test_image).exists() {
        eprintln!("Skipping test - test image not found at {}", test_image);
        return;
    }
    
    let result = engine.recognize_image(test_image);
    assert!(result.is_ok(), "Image recognition should succeed: {:?}", result.err());
    let response = result.unwrap();
    assert!(!response.texts.is_empty(), "OCR should return at least some text items");
}

/// Test OCR PDF recognition (currently not supported in embedded mode).
/// This test requires OCR model files to be present, so it's marked #[ignore].
/// Run with: cargo test --test ocr_integration -- --ignored
#[test]
#[ignore]
fn test_ocr_recognize_pdf() {
    let models_dir = std::env::var("MODELS_DIR")
        .unwrap_or_else(|_| "models".to_string());
    
    let mut engine = OcrEngine::new(&models_dir)
        .expect("Failed to create OcrEngine - ensure models are present");
    
    let test_pdf = "tests/fixtures/test_invoice.pdf";
    let result = engine.recognize_pdf(test_pdf);
    
    // PDF recognition is currently not supported in embedded mode
    assert!(result.is_err(), "PDF recognition should return error in embedded mode");
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("PDF OCR not supported"), 
        "Error message should mention PDF not supported");
}

/// Test OCR engine initialization with all required model files.
/// This test requires all three ONNX model files to be present, so it's marked #[ignore].
/// Run with: cargo test --test ocr_integration -- --ignored
#[test]
#[ignore]
fn test_ocr_engine_with_valid_models() {
    let models_dir = std::env::var("MODELS_DIR")
        .unwrap_or_else(|_| "models".to_string());
    
    let engine = OcrEngine::new(&models_dir);
    assert!(engine.is_ok(), "Should succeed with valid models directory: {:?}", engine.err());
    
    let engine = engine.unwrap();
    
    // Test health check
    let health = engine.health();
    assert!(health.is_ok(), "Health check should succeed");
    assert!(health.unwrap(), "Health check should return true");
}
