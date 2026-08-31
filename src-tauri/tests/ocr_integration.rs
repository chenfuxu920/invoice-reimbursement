use invoice_reimbursement_lib::ocr::OcrEngine;

/// Test that OcrEngine::new() degrades gracefully when models directory doesn't exist:
/// returns an uninitialized engine (Ok) whose health() is false.
#[test]
fn test_ocr_engine_new_missing_models_dir() {
    let result = OcrEngine::new("/nonexistent/models/dir");
    assert!(
        result.is_ok(),
        "Missing models dir should degrade gracefully, not error: {:?}",
        result.err()
    );
    let health = result.unwrap().health().unwrap();
    assert!(!health, "Engine without models should report unhealthy");
}

/// Test that OcrEngine::new() degrades gracefully when models directory exists
/// but model files are missing: returns an uninitialized engine (Ok).
#[test]
fn test_ocr_engine_new_empty_models_dir() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let models_path = temp_dir.path().to_str().unwrap();

    let result = OcrEngine::new(models_path);
    assert!(
        result.is_ok(),
        "Missing model files should degrade gracefully, not error: {:?}",
        result.err()
    );
    let health = result.unwrap().health().unwrap();
    assert!(!health, "Engine without models should report unhealthy");
}

/// Test OCR engine initialization with all required model files.
/// This test requires model files (mnn) to be present in `models/`, so it's marked #[ignore].
/// Run with: cargo test --test ocr_integration -- --ignored
#[test]
#[ignore]
fn test_ocr_engine_with_valid_models() {
    let models_dir = std::env::var("MODELS_DIR").unwrap_or_else(|_| "models".to_string());

    let engine = OcrEngine::new(&models_dir);
    assert!(
        engine.is_ok(),
        "Should succeed with valid models directory: {:?}",
        engine.err()
    );

    let engine = engine.unwrap();

    // Test health check
    let health = engine.health();
    assert!(health.is_ok(), "Health check should succeed");
    assert!(health.unwrap(), "Health check should return true");
}
