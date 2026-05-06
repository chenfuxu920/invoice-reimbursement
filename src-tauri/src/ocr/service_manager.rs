use std::process::{Child, Command};
use std::path::PathBuf;

pub struct OcrServiceManager {
    process: Option<Child>,
}

impl OcrServiceManager {
    pub fn new() -> Self {
        Self { process: None }
    }

    pub fn start(&mut self, project_dir: &str) -> Result<(), String> {
        if self.is_running() {
            return Ok(());
        }

        let python_path = PathBuf::from(project_dir)
            .join("ocr-service")
            .join("venv")
            .join("bin")
            .join("python");

        let ocr_service_dir = PathBuf::from(project_dir)
            .join("ocr-service");

        let child = Command::new(python_path)
            .arg("-m")
            .arg("uvicorn")
            .arg("main:app")
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg("8080")
            .current_dir(ocr_service_dir)
            .spawn()
            .map_err(|e| format!("Failed to start OCR service: {}", e))?;

        self.process = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        if let Some(ref mut child) = self.process {
            child.kill().map_err(|e| format!("Failed to stop OCR service: {}", e))?;
            let _ = child.wait(); // reap the process to avoid zombies
            self.process = None;
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }
}

impl Drop for OcrServiceManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
