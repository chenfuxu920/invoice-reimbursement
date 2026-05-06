# OCR 嵌入式改造实施计划

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task via OpenCode.

**Goal:** 将 OCR 服务从外部 Python HTTP 服务改造为 paddle-ocr-rs 嵌入式方案，直接在 Rust 后端调用 ONNX Runtime 推理，无需 Python 运行时。

**Architecture:** 用 paddle-ocr-rs crate 替换 reqwest HTTP 客户端 + Python 服务管理器。模型文件打包进应用资源，运行时通过 Tauri resource_dir() 加载。删除 ocr-service/ 目录和 service_manager.rs。

**Tech Stack:** paddle-ocr-rs 0.7.x + ort 2.0.0-rc + ONNX Runtime + PP-OCRv4 ONNX 模型

---

### Task 1: 添加 paddle-ocr-rs 依赖到 Cargo.toml

**Objective:** 在 Rust 后端引入 paddle-ocr-rs 和相关依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: 编辑 Cargo.toml，添加依赖**

在 `[dependencies]` 中添加：
```toml
paddle-ocr-rs = "0.7"
ort = { version = "2", features = ["download-binaries", "copy-dylibs"] }
image = "0.25"
```

注意：移除 `reqwest` 的 `json` 和 `multipart` features（OCR 不再需要 HTTP），但保留 `reqwest` 基础依赖以防其他地方使用。如果 reqwest 只用于 OCR，可以完全移除。

**Step 2: 验证依赖可下载**

Run: `cd ~/projects/invoice-reimbursement/src-tauri && cargo check 2>&1 | head -50`

Expected: 可能需要较长时间下载 ort 预编译库，最终应无 resolve 错误

**Step 3: Commit**

```bash
cd ~/projects/invoice-reimbursement && git add src-tauri/Cargo.toml && git commit -m "feat: add paddle-ocr-rs and ort dependencies"
```

---

### Task 2: 下载 PP-OCR ONNX 模型文件

**Objective:** 下载 PP-OCRv4 det/cls/rec ONNX 模型文件到项目资源目录

**Files:**
- Create: `src-tauri/models/` 目录
- Download: 3个 ONNX 模型文件 + 字典文件

**Step 1: 创建模型目录并下载模型**

```bash
mkdir -p ~/projects/invoice-reimbursement/src-tauri/models
cd ~/projects/invoice-reimbursement/src-tauri/models

# PP-OCRv4 检测模型
wget -q https://raw.githubusercontent.com/RapidAI/RapidOCR/main/models/ch_PP-OCRv4_det_infer.onnx -O ch_PP-OCRv4_det_infer.onnx

# PP-OCRv4 分类模型
wget -q https://raw.githubusercontent.com/RapidAI/RapidOCR/main/models/ch_ppocr_mobile_v2.0_cls_infer.onnx -O ch_ppocr_mobile_v2.0_cls_infer.onnx

# PP-OCRv4 识别模型
wget -q https://raw.githubusercontent.com/RapidAI/RapidOCR/main/models/ch_PP-OCRv4_rec_infer.onnx -O ch_PP-OCRv4_rec_infer.onnx

# 字典文件
wget -q https://raw.githubusercontent.com/RapidAI/RapidOCR/main/models/ppocr_keys_v1.txt -O ppocr_keys_v1.txt
```

注意：如果 RapidAI 仓库模型路径不同，可从 paddle-ocr-rs 项目的 GitHub Releases 或 HuggingFace 下载。

**Step 2: 验证模型文件**

```bash
ls -lh ~/projects/invoice-reimbursement/src-tauri/models/
```

Expected: 3个 .onnx 文件 + 1个 .txt 文件，总计约 12-16MB

**Step 3: 配置 tauri.conf.json 打包模型资源**

在 `tauri.conf.json` 的 `bundle` 下添加 `resources`：
```json
"resources": [
  "models/*.onnx",
  "models/*.txt"
]
```

**Step 4: 添加 .gitignore 排除大文件**

在 `src-tauri/models/` 下创建 `.gitignore`：
```
*.onnx
```

**Step 5: Commit**

```bash
cd ~/projects/invoice-reimbursement && git add src-tauri/models/ppocr_keys_v1.txt src-tauri/models/.gitignore src-tauri/tauri.conf.json && git commit -m "feat: add PP-OCR model resources and bundle config"
```

---

### Task 3: 重写 ocr/client.rs 为嵌入式 OCR 引擎

**Objective:** 用 paddle-ocr-rs 替换 HTTP 客户端，实现直接在进程内 OCR 推理

**Files:**
- Rewrite: `src-tauri/src/ocr/client.rs` → `src-tauri/src/ocr/engine.rs`

**Step 1: 创建新文件 ocr/engine.rs**

保留 OcrTextItem / OcrImageResponse / OcrPdfResponse 数据结构（这些是公共接口，被 parser 和 lib.rs 使用）。
将 OcrClient 替换为 OcrEngine，内部持有 paddle-ocr-rs 的 PaddleOcr 实例。

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

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
    // paddle-ocr-rs 实例（需要在 init 时创建）
    ocr: paddle_ocr_rs::PaddleOcr,
}

impl OcrEngine {
    /// 从模型目录初始化 OCR 引擎
    pub fn new(models_dir: &str) -> Result<Self, String> {
        let det_model = Path::new(models_dir).join("ch_PP-OCRv4_det_infer.onnx");
        let cls_model = Path::new(models_dir).join("ch_ppocr_mobile_v2.0_cls_infer.onnx");
        let rec_model = Path::new(models_dir).join("ch_PP-OCRv4_rec_infer.onnx");
        let dict_path = Path::new(models_dir).join("ppocr_keys_v1.txt");

        // 检查文件存在
        for (name, path) in [("det", &det_model), ("cls", &cls_model), ("rec", &rec_model), ("dict", &dict_path)] {
            if !path.exists() {
                return Err(format!("OCR model file not found: {} ({})", name, path.display()));
            }
        }

        let ocr = paddle_ocr_rs::PaddleOcr::new(
            det_model.to_str().unwrap(),
            cls_model.to_str().unwrap(),
            rec_model.to_str().unwrap(),
            dict_path.to_str().unwrap(),
        ).map_err(|e| format!("Failed to init PaddleOCR: {:?}", e))?;

        Ok(Self { ocr })
    }

    /// 健康检查 - 引擎已内置，始终可用
    pub async fn health(&self) -> Result<bool, String> {
        Ok(true)
    }

    /// 识别图片
    pub async fn recognize_image(&self, file_path: &str) -> Result<OcrImageResponse, String> {
        let img = image::open(file_path)
            .map_err(|e| format!("Failed to open image {}: {}", file_path, e))?;

        let result = self.ocr.ocr(&img)
            .map_err(|e| format!("OCR failed: {:?}", e))?;

        let texts = result.iter().map(|item| {
            OcrTextItem {
                text: item.text.clone(),
                confidence: item.confidence as f64,
                box_coords: None, // paddle-ocr-rs 返回的坐标可后续补充
            }
        }).collect();

        Ok(OcrImageResponse { texts })
    }

    /// 识别 PDF（逐页转图片后 OCR）
    pub async fn recognize_pdf(&self, file_path: &str) -> Result<OcrPdfResponse, String> {
        // PDF 转图片需要 pdf-render 依赖，先用简单实现
        // 将 PDF 第一页作为图片处理，后续可增强
        Err("PDF OCR will be implemented with pdf-render dependency".to_string())
    }
}
```

注意：
- paddle-ocr-rs 的具体 API 需要根据实际 crate 文档调整
- `ocr.ocr()` 返回类型需要根据 paddle-ocr-rs 源码确认
- PDF 支持后续可添加 `pdf-render` crate

**Step 2: 删除 ocr/client.rs**

```bash
rm ~/projects/invoice-reimbursement/src-tauri/src/ocr/client.rs
```

**Step 3: 更新 ocr/mod.rs**

```rust
pub mod engine;
// pub mod service_manager;  -- 删除此模块

pub use engine::{OcrEngine, OcrTextItem, OcrImageResponse, OcrPdfResponse};
```

**Step 4: 删除 ocr/service_manager.rs**

```bash
rm ~/projects/invoice-reimbursement/src-tauri/src/ocr/service_manager.rs
```

**Step 5: cargo check 验证编译**

Run: `cd ~/projects/invoice-reimbursement/src-tauri && cargo check 2>&1 | tail -20`

Expected: 可能有 lib.rs 编译错误（因为 OcrClient→OcrEngine 引用变更），这是预期的，Task 4 修复

---

### Task 4: 重构 lib.rs 使用 OcrEngine

**Objective:** 将 lib.rs 中的 OcrClient + OcrServiceManager 替换为 OcrEngine

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: 修改 import 和 AppState**

```rust
use ocr::{OcrEngine, OcrTextItem};
// 移除: use ocr::{OcrClient, OcrServiceManager, OcrTextItem};

struct AppState {
    ocr_engine: AsyncMutex<OcrEngine>,
    // 移除: ocr_service: Mutex<OcrServiceManager>,
}
```

**Step 2: 修改 OCR 相关命令**

- `ocr_health`: 改为调用 `engine.health()`
- `ocr_recognize_image`: 改为 `engine.recognize_image()`
- `ocr_recognize_pdf`: 改为 `engine.recognize_pdf()`
- 删除 `start_ocr_service`、`stop_ocr_service`、`is_ocr_service_running` 三个命令
- `recognize_invoice`: 改为 `engine.recognize_image()` / `engine.recognize_pdf()`
- `recognize_itinerary`: 同上

**Step 3: 修改 AppState 初始化和 setup**

```rust
.manage(AppState {
    ocr_engine: AsyncMutex::new(
        OcrEngine::new(models_dir).expect("Failed to init OCR engine")
    ),
})
```

models_dir 通过 `app.path().resource_dir()` 获取，在 `.setup()` 中初始化。

**Step 4: 移除 setup 中的 OCR 服务自动启动和 on_window_event 中的停止逻辑**

**Step 5: 更新 invoke_handler**

移除 `start_ocr_service`、`stop_ocr_service`、`is_ocr_service_running`。

**Step 6: cargo check 验证**

Run: `cd ~/projects/invoice-reimbursement/src-tauri && cargo check 2>&1`

Expected: 编译通过

**Step 7: Commit**

```bash
cd ~/projects/invoice-reimbursement && git add -A && git commit -m "refactor: replace OCR HTTP client with embedded OcrEngine"
```

---

### Task 5: 移除 Python OCR 服务和不必要的依赖

**Objective:** 清理不再需要的 Python 服务代码和 Rust 依赖

**Files:**
- Delete: `ocr-service/` 整个目录
- Modify: `src-tauri/Cargo.toml` (移除不再需要的 reqwest)

**Step 1: 删除 Python OCR 服务目录**

```bash
rm -rf ~/projects/invoice-reimbursement/ocr-service/
rm -rf ~/projects/invoice-reimbursement/venv/
```

**Step 2: 检查 reqwest 是否还被使用**

```bash
cd ~/projects/invoice-reimbursement/src-tauri && grep -r "reqwest" src/ --include="*.rs"
```

如果没有其他引用，从 Cargo.toml 移除 reqwest 依赖。

**Step 3: 移除其他不再需要的依赖**

- `tokio` 的 `full` features 可缩减（如果不再需要异步 HTTP），但保留以备 Tauri 使用
- 移除与 Python 服务管理相关的依赖

**Step 4: cargo check 验证**

**Step 5: Commit**

```bash
cd ~/projects/invoice-reimbursement && git add -A && git commit -m "chore: remove Python OCR service and unused dependencies"
```

---

### Task 6: 更新测试

**Objective:** 修复因 OCR 改造导致的测试编译错误，确保所有 91 个 Rust 测试通过

**Files:**
- Modify: `src-tauri/tests/` 下的测试文件
- Modify: `src-tauri/src/` 下的单元测试模块

**Step 1: 运行测试看哪些失败**

```bash
cd ~/projects/invoice-reimbursement/src-tauri && cargo test 2>&1 | tail -30
```

**Step 2: 修复编译错误**

- 将 OcrClient 相关的测试改为 OcrEngine
- 移除 service_manager 相关的测试
- 添加 OcrEngine::new 的单元测试（需要 mock 或跳过如果无模型文件）

**Step 3: 验证所有测试通过**

```bash
cd ~/projects/invoice-reimbursement/src-tauri && cargo test 2>&1
```

Expected: 所有测试通过

**Step 4: Commit**

```bash
cd ~/projects/invoice-reimbursement && git add -A && git commit -m "test: update tests for embedded OCR engine"
```

---

### Task 7: Release 构建验证

**Objective:** 确认改造后应用可以成功 release 构建，检查包体积

**Files:**
- Modify: `docs/BUILD.md` (更新构建文档)

**Step 1: Release 构建**

```bash
cd ~/projects/invoice-reimbursement && npm run tauri build 2>&1 | tail -20
```

**Step 2: 检查构建产物和体积**

```bash
ls -lh ~/projects/invoice-reimbursement/src-tauri/target/release/bundle/
```

**Step 3: 更新 BUILD.md**

记录新的依赖、模型文件路径、构建步骤变化。

**Step 4: Commit**

```bash
cd ~/projects/invoice-reimbursement && git add -A && git commit -m "docs: update BUILD.md for embedded OCR"
```

---

## 改造前后对比

| 项目 | 改造前 | 改造后 |
|------|--------|--------|
| OCR 运行方式 | Python uvicorn HTTP 服务 | Rust 进程内直接调用 |
| 启动流程 | 需启动 Python 子进程 | 自动初始化，无需管理 |
| 依赖 | Python 3 + PaddleOCR + venv | paddle-ocr-rs + ort (自动下载) |
| 内存占用 | 500MB-1.5GB | 200-400MB |
| 速度 | 3-10s/张 | 1-4s/张 |
| 打包体积 | 需打包 Python 运行时 | 增量 ~30-50MB (ONNX Runtime + 模型) |
| 前端调用 | invoke → HTTP → Python | invoke → Rust 直调 |
