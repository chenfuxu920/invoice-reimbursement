use invoice_reimbursement_lib::ocr::OcrClient;

/// Test that OcrClient can be constructed with a given base URL.
#[test]
fn test_ocr_client_construction() {
    let client = OcrClient::new("http://127.0.0.1:8080");
    // If construction fails, this test won't compile or will panic.
    // Just verifying the type can be created.
    let _ = client;
}

/// Test OCR service health check.
/// This test requires the OCR service to be running, so it's marked #[ignore].
/// Run with: cargo test --test ocr_integration -- --ignored
#[tokio::test]
#[ignore]
async fn test_ocr_health_check() {
    let client = OcrClient::new("http://127.0.0.1:8080");
    let result = client.health().await;
    assert!(result.is_ok(), "Health check should succeed when OCR service is running");
    assert!(result.unwrap(), "Health check should return true");
}

/// Test OCR image recognition.
/// This test requires the OCR service to be running and a test image file.
/// Run with: cargo test --test ocr_integration -- --ignored
#[tokio::test]
#[ignore]
async fn test_ocr_recognize_image() {
    let client = OcrClient::new("http://127.0.0.1:8080");
    // Use a test image that should exist if OCR service is properly set up
    let result = client.recognize_image("tests/fixtures/test_invoice.png").await;
    assert!(result.is_ok(), "Image recognition should succeed: {:?}", result.err());
    let response = result.unwrap();
    assert!(!response.texts.is_empty(), "OCR should return at least some text items");
}

/// Test OCR PDF recognition.
/// This test requires the OCR service to be running and a test PDF file.
/// Run with: cargo test --test ocr_integration -- --ignored
#[tokio::test]
#[ignore]
async fn test_ocr_recognize_pdf() {
    let client = OcrClient::new("http://127.0.0.1:8080");
    let result = client.recognize_pdf("tests/fixtures/test_invoice.pdf").await;
    assert!(result.is_ok(), "PDF recognition should succeed: {:?}", result.err());
    let response = result.unwrap();
    assert!(!response.pages.is_empty(), "OCR should return at least one page");
}

/// Test OCR health check with unreachable service returns error.
#[tokio::test]
async fn test_ocr_health_unreachable_service() {
    let client = OcrClient::new("http://127.0.0.1:19999");
    let result = client.health().await;
    assert!(result.is_err(), "Health check should fail when service is unreachable");
}
