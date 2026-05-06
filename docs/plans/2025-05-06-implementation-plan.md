# 发票报销自动化系统 实施计划

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** 构建一个 Tauri 桌面应用，实现发票 OCR 识别、支付账单匹配、报销表单/PDF 自动生成。

**Architecture:** Tauri 2.x (Rust 后端 + Vue 3 前端)，Python 独立 OCR 服务通过 HTTP 通信。Rust 负责文件解析、匹配引擎、PDF 生成；Vue 3 负责 UI 交互；Python 负责 OCR 识别。

**Tech Stack:** Tauri 2.x, Vue 3 + TypeScript + Vite + TailwindCSS + Pinia, Rust, Python (PaddleOCR/RapidOCR), printpdf/genpdf

---

## 模块划分

```
Phase 1: 项目骨架搭建 (Task 1-3)
Phase 2: OCR 识别服务 (Task 4-7)
Phase 3: 发票解析模块 (Task 8-12)
Phase 4: 支付账单解析 (Task 13-15)
Phase 5: 匹配引擎 (Task 16-19)
Phase 6: PDF 生成模块 (Task 20-24)
Phase 7: 前端界面 (Task 25-32)
Phase 8: 集成测试与打包 (Task 33-35)
```

---

## Phase 1: 项目骨架搭建

### Task 1: 初始化 Tauri 2.x + Vue 3 项目

**Objective:** 搭建项目骨架，确认 Tauri + Vue 3 能正常启动。

**Files:**
- Create: `~/projects/invoice-reimbursement/` 整个 Tauri 项目结构

**Step 1: 创建 Tauri 项目**

```bash
cd ~/projects
npm create tauri-app@latest invoice-reimbursement -- \
  --template vue-ts \
  --manager npm \
  --frontend-flavor vue-ts
```

**Step 2: 安装前端依赖**

```bash
cd ~/projects/invoice-reimbursement
npm install
npm install -D tailwindcss @tailwindcss/vite
npm install pinia vue-router
npm install @tauri-apps/api @tauri-apps/plugin-dialog @tauri-apps/plugin-fs
```

**Step 3: 配置 TailwindCSS**

创建 `src/style.css`:
```css
@import "tailwindcss";
```

更新 `vite.config.ts`:
```ts
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [vue(), tailwindcss()],
});
```

**Step 4: 配置 Pinia**

创建 `src/stores/index.ts`:
```ts
import { createPinia } from 'pinia'
export const pinia = createPinia()
```

在 `src/main.ts` 中注册:
```ts
import { pinia } from './stores'
app.use(pinia)
```

**Step 5: 验证启动**

```bash
cd ~/projects/invoice-reimbursement
npm run tauri dev
```

Expected: 应用窗口正常打开，显示 Vue 默认页面。

**Step 6: 提交**

```bash
git init
git add .
git commit -m "feat: initialize Tauri 2.x + Vue 3 project skeleton"
```

---

### Task 2: 定义 Rust 核心数据结构

**Objective:** 定义发票、支付记录、匹配结果等核心数据模型。

**Files:**
- Create: `src-tauri/src/models/mod.rs`
- Create: `src-tauri/src/models/invoice.rs`
- Create: `src-tauri/src/models/payment.rs`
- Create: `src-tauri/src/models/match_result.rs`
- Create: `src-tauri/src/models/reimbursement.rs`
- Modify: `src-tauri/src/main.rs` (添加 mod models)

**Step 1: 创建发票数据模型**

`src-tauri/src/models/invoice.rs`:
```rust
use serde::{Deserialize, Serialize};
use chrono::NaiveDate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvoiceCategory {
    Train,          // 高铁/车船票
    Flight,         // 飞机票
    TicketChange,   // 退改签/保险费
    CityTransport,  // 市内交通
    Hotel,          // 住宿费
    Meal,           // 餐饮费
    Other,          // 其他
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvoiceSource {
    Photo(String),       // 照片路径
    Pdf(String),         // PDF路径
    Link(String),        // 发票链接
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub invoice_number: String,      // 发票号码
    pub amount: f64,                  // 金额
    pub seller_name: String,          // 销售方名称
    pub item_name: String,            // 项目名称
    pub date: NaiveDate,              // 开票日期
    pub category: InvoiceCategory,    // 自动识别的类别
    pub source: InvoiceSource,        // 来源
    pub itineraries: Vec<Itinerary>,  // 行程（打车场景）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Itinerary {
    pub date_time: String,     // 行程时间
    pub provider: String,      // 服务商（滴滴/高德）
    pub pickup: String,        // 上车点
    pub dropoff: String,       // 下车点
    pub amount: f64,           // 行程金额
}
```

**Step 2: 创建支付记录模型**

`src-tauri/src/models/payment.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentSource {
    Wechat,     // 微信
    Alipay,     // 支付宝
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRecord {
    pub id: String,
    pub transaction_id: String,      // 交易单号
    pub transaction_time: String,    // 交易时间
    pub amount: f64,                 // 交易金额
    pub merchant_name: String,       // 商户名称
    pub source: PaymentSource,       // 来源
    pub category: String,            // 交易类型
}
```

**Step 3: 创建匹配结果模型**

`src-tauri/src/models/match_result.rs`:
```rust
use serde::{Deserialize, Serialize};
use super::invoice::Invoice;
use super::payment::PaymentRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    OneToOne,            // 1张发票 → 1笔支付
    OneToMany,           // 1张发票 → 多笔支付（打车）
    Unmatched,           // 未匹配
    ManualConfirmed,     // 手动确认
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub invoice_id: String,
    pub invoice: Invoice,
    pub payment_ids: Vec<String>,
    pub payments: Vec<PaymentRecord>,
    pub match_type: MatchType,
    pub confidence: f64,             // 匹配置信度 0-1
    pub amount_diff: f64,            // 金额差异
}
```

**Step 4: 创建报销汇总模型**

`src-tauri/src/models/reimbursement.rs`:
```rust
use serde::{Deserialize, Serialize};
use super::invoice::InvoiceCategory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: InvoiceCategory,
    pub count: usize,                // 单据张数
    pub total_amount: f64,           // 申报金额
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReimbursementForm {
    pub name: String,                // 姓名
    pub department: String,          // 部职别
    pub travel_start: String,        // 出差开始日期
    pub travel_end: String,          // 出差结束日期
    pub companions: usize,           // 同行人数
    pub summaries: Vec<CategorySummary>,
    pub total_amount: f64,           // 总计
}
```

**Step 5: 注册模块**

`src-tauri/src/models/mod.rs`:
```rust
pub mod invoice;
pub mod payment;
pub mod match_result;
pub mod reimbursement;
```

在 `src-tauri/src/main.rs` 顶部添加:
```rust
mod models;
```

**Step 6: 验证编译**

```bash
cd ~/projects/invoice-reimbursement
# 先在 Cargo.toml 添加依赖: serde, chrono
npm run tauri build 2>&1 | head -20
```

Expected: 编译通过（可能有未使用的警告，但不报错）。

**Step 7: 提交**

```bash
git add .
git commit -m "feat: define core data models for invoice, payment, matching"
```

---

### Task 3: 定义前端 TypeScript 数据类型

**Objective:** 前端 TypeScript 类型与 Rust 模型对齐。

**Files:**
- Create: `src/types/invoice.ts`
- Create: `src/types/payment.ts`
- Create: `src/types/match.ts`
- Create: `src/types/reimbursement.ts`
- Create: `src/types/index.ts`

**Step 1: 创建类型定义**

`src/types/invoice.ts`:
```ts
export type InvoiceCategory =
  | 'Train'
  | 'Flight'
  | 'TicketChange'
  | 'CityTransport'
  | 'Hotel'
  | 'Meal'
  | 'Other'

export interface InvoiceSource {
  type: 'Photo' | 'Pdf' | 'Link'
  path: string
}

export interface Itinerary {
  date_time: string
  provider: string
  pickup: string
  dropoff: string
  amount: number
}

export interface Invoice {
  id: string
  invoice_number: string
  amount: number
  seller_name: string
  item_name: string
  date: string
  category: InvoiceCategory
  source: InvoiceSource
  itineraries: Itinerary[]
}

export const CATEGORY_LABELS: Record<InvoiceCategory, string> = {
  Train: '高铁/车船票',
  Flight: '飞机票',
  TicketChange: '退改签/保险费',
  CityTransport: '市内交通',
  Hotel: '住宿费',
  Meal: '餐饮费',
  Other: '其他',
}
```

`src/types/payment.ts`:
```ts
export type PaymentSource = 'Wechat' | 'Alipay'

export interface PaymentRecord {
  id: string
  transaction_id: string
  transaction_time: string
  amount: number
  merchant_name: string
  source: PaymentSource
  category: string
}
```

`src/types/match.ts`:
```ts
import type { Invoice } from './invoice'
import type { PaymentRecord } from './payment'

export type MatchType = 'OneToOne' | 'OneToMany' | 'Unmatched' | 'ManualConfirmed'

export interface MatchResult {
  invoice_id: string
  invoice: Invoice
  payment_ids: string[]
  payments: PaymentRecord[]
  match_type: MatchType
  confidence: number
  amount_diff: number
}
```

`src/types/reimbursement.ts`:
```ts
import type { InvoiceCategory } from './invoice'

export interface CategorySummary {
  category: InvoiceCategory
  count: number
  total_amount: number
}

export interface ReimbursementForm {
  name: string
  department: string
  travel_start: string
  travel_end: string
  companions: number
  summaries: CategorySummary[]
  total_amount: number
}
```

`src/types/index.ts`:
```ts
export * from './invoice'
export * from './payment'
export * from './match'
export * from './reimbursement'
```

**Step 2: 验证编译**

```bash
cd ~/projects/invoice-reimbursement
npx tsc --noEmit
```

Expected: 无类型错误。

**Step 3: 提交**

```bash
git add .
git commit -m "feat: add frontend TypeScript type definitions"
```

---

## Phase 2: OCR 识别服务

### Task 4: 搭建 Python OCR 服务骨架

**Objective:** 创建 Python FastAPI 服务，提供 OCR 识别接口。

**Files:**
- Create: `ocr-service/main.py`
- Create: `ocr-service/requirements.txt`
- Create: `ocr-service/ocr_engine.py`

**Step 1: 创建项目结构**

```bash
mkdir -p ~/projects/invoice-reimbursement/ocr-service
```

`ocr-service/requirements.txt`:
```
fastapi==0.115.0
uvicorn==0.32.0
python-multipart==0.0.12
paddleocr==2.9.0
paddlepaddle==3.0.0
Pillow==11.0.0
```

**Step 2: 创建 OCR 引擎封装**

`ocr-service/ocr_engine.py`:
```python
from paddleocr import PaddleOCR
from typing import Optional

_ocr_engine: Optional[PaddleOCR] = None

def get_ocr() -> PaddleOCR:
    global _ocr_engine
    if _ocr_engine is None:
        _ocr_engine = PaddleOCR(
            use_angle_cls=True,
            lang='ch',
            show_log=False,
        )
    return _ocr_engine

def recognize_image(image_bytes: bytes) -> list[dict]:
    """识别图片中的文字，返回结构化结果"""
    import numpy as np
    from PIL import Image
    import io

    img = Image.open(io.BytesIO(image_bytes))
    img_array = np.array(img)

    ocr = get_ocr()
    result = ocr.ocr(img_array, cls=True)

    texts = []
    if result and result[0]:
        for line in result[0]:
            box = line[0]        # 坐标
            text = line[1][0]    # 文字
            confidence = line[1][1]  # 置信度
            texts.append({
                'text': text,
                'confidence': float(confidence),
                'box': box,
            })
    return texts
```

**Step 3: 创建 FastAPI 服务**

`ocr-service/main.py`:
```python
from fastapi import FastAPI, UploadFile, File
from ocr_engine import recognize_image

app = FastAPI(title="Invoice OCR Service", version="0.1.0")

@app.get("/health")
async def health():
    return {"status": "ok"}

@app.post("/ocr/image")
async def ocr_image(file: UploadFile = File(...)):
    """识别上传的图片"""
    image_bytes = await file.read()
    result = recognize_image(image_bytes)
    return {"texts": result}

@app.post("/ocr/pdf")
async def ocr_pdf(file: UploadFile = File(...)):
    """识别上传的 PDF（转为图片后 OCR）"""
    from pdf2image import convert_from_bytes
    import io

    pdf_bytes = await file.read()
    images = convert_from_bytes(pdf_bytes)

    all_texts = []
    for i, img in enumerate(images):
        buf = io.BytesIO()
        img.save(buf, format='PNG')
        result = recognize_image(buf.getvalue())
        all_texts.append({
            'page': i + 1,
            'texts': result,
        })
    return {"pages": all_texts}
```

**Step 4: 安装依赖并测试**

```bash
cd ~/projects/invoice-reimbursement/ocr-service
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
uvicorn main:app --host 127.0.0.1 --port 8080 &
curl http://127.0.0.1:8080/health
```

Expected: `{"status":"ok"}`

**Step 5: 提交**

```bash
git add .
git commit -m "feat: create Python OCR service with PaddleOCR"
```

---

### Task 5: 实现 Rust OCR HTTP 客户端

**Objective:** Tauri 后端通过 HTTP 调用 Python OCR 服务。

**Files:**
- Create: `src-tauri/src/ocr/client.rs`
- Create: `src-tauri/src/ocr/mod.rs`
- Modify: `src-tauri/Cargo.toml` (添加 reqwest 依赖)
- Modify: `src-tauri/src/main.rs` (添加 mod ocr)

**Step 1: 添加 Cargo 依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中添加:
```toml
reqwest = { version = "0.12", features = ["json", "multipart"] }
tokio = { version = "1", features = ["full"] }
base64 = "0.22"
```

**Step 2: 创建 OCR 客户端**

`src-tauri/src/ocr/client.rs`:
```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct OcrTextItem {
    pub text: String,
    pub confidence: f64,
    pub box_coords: Vec<Vec<f64>>,
}

#[derive(Debug, Deserialize)]
pub struct OcrImageResponse {
    pub texts: Vec<OcrTextItem>,
}

#[derive(Debug, Deserialize)]
pub struct OcrPageResult {
    pub page: u32,
    pub texts: Vec<OcrTextItem>,
}

#[derive(Debug, Deserialize)]
pub struct OcrPdfResponse {
    pub pages: Vec<OcrPageResult>,
}

pub struct OcrClient {
    client: Client,
    base_url: String,
}

impl OcrClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    pub async fn health(&self) -> Result<bool, String> {
        let resp = self.client
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        Ok(resp.status().is_success())
    }

    pub async fn recognize_image(&self, file_path: &str) -> Result<OcrImageResponse, String> {
        let file_bytes = std::fs::read(file_path).map_err(|e| e.to_string())?;
        let file_name = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("image/png")
            .map_err(|e| e.to_string())?;

        let form = reqwest::multipart::Form::new()
            .part("file", part);

        let resp = self.client
            .post(format!("{}/ocr/image", self.base_url))
            .multipart(form)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        resp.json::<OcrImageResponse>()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn recognize_pdf(&self, file_path: &str) -> Result<OcrPdfResponse, String> {
        let file_bytes = std::fs::read(file_path).map_err(|e| e.to_string())?;
        let file_name = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("application/pdf")
            .map_err(|e| e.to_string())?;

        let form = reqwest::multipart::Form::new()
            .part("file", part);

        let resp = self.client
            .post(format!("{}/ocr/pdf", self.base_url))
            .multipart(form)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        resp.json::<OcrPdfResponse>()
            .await
            .map_err(|e| e.to_string())
    }
}
```

`src-tauri/src/ocr/mod.rs`:
```rust
pub mod client;
pub use client::OcrClient;
```

**Step 3: 注册模块并验证编译**

在 `src-tauri/src/main.rs` 添加:
```rust
mod ocr;
```

```bash
cd ~/projects/invoice-reimbursement
npm run tauri build 2>&1 | tail -5
```

Expected: 编译通过。

**Step 4: 提交**

```bash
git add .
git commit -m "feat: add Rust OCR HTTP client for Python service"
```

---

### Task 6: 注册 Tauri 命令 — OCR 识别

**Objective:** 将 OCR 功能暴露为 Tauri 命令，前端可调用。

**Files:**
- Modify: `src-tauri/src/main.rs` (添加 Tauri commands)

**Step 1: 添加 Tauri 命令**

在 `src-tauri/src/main.rs` 中添加:
```rust
use ocr::OcrClient;
use std::sync::Mutex;

struct AppState {
    ocr_client: Mutex<OcrClient>,
}

#[tauri::command]
async fn ocr_health(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let client = state.ocr_client.lock().map_err(|e| e.to_string())?;
    client.health().await
}

#[tauri::command]
async fn ocr_recognize_image(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<serde_json::Value, String> {
    let client = state.ocr_client.lock().map_err(|e| e.to_string())?;
    let result = client.recognize_image(&file_path).await?;
    Ok(serde_json::to_value(result).map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn ocr_recognize_pdf(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<serde_json::Value, String> {
    let client = state.ocr_client.lock().map_err(|e| e.to_string())?;
    let result = client.recognize_pdf(&file_path).await?;
    Ok(serde_json::to_value(result).map_err(|e| e.to_string())?)
}
```

**Step 2: 注册命令到 Tauri Builder**

修改 `main()` 中的 Builder:
```rust
fn main() {
    tauri::Builder::default()
        .manage(AppState {
            ocr_client: Mutex::new(OcrClient::new("http://127.0.0.1:8080")),
        })
        .invoke_handler(tauri::generate_handler![
            ocr_health,
            ocr_recognize_image,
            ocr_recognize_pdf,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: 验证编译**

```bash
npm run tauri build 2>&1 | tail -5
```

Expected: 编译通过。

**Step 4: 提交**

```bash
git add .
git commit -m "feat: register Tauri commands for OCR recognition"
```

---

### Task 7: 实现 OCR 服务自动启停管理

**Objective:** Tauri 应用启动时自动启动 Python OCR 服务，退出时自动关闭。

**Files:**
- Create: `src-tauri/src/ocr/service_manager.rs`
- Modify: `src-tauri/src/ocr/mod.rs`

**Step 1: 创建服务管理器**

`src-tauri/src/ocr/service_manager.rs`:
```rust
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
        let python_path = PathBuf::from(project_dir)
            .join("ocr-service")
            .join("venv")
            .join("bin")
            .join("python");

        let main_path = PathBuf::from(project_dir)
            .join("ocr-service")
            .join("main.py");

        let child = Command::new(python_path)
            .arg("-m")
            .arg("uvicorn")
            .arg("main:app")
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg("8080")
            .current_dir(PathBuf::from(project_dir).join("ocr-service"))
            .spawn()
            .map_err(|e| format!("Failed to start OCR service: {}", e))?;

        self.process = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        if let Some(ref mut child) = self.process {
            child.kill().map_err(|e| format!("Failed to stop OCR service: {}", e))?;
            self.process = None;
        }
        Ok(())
    }
}

impl Drop for OcrServiceManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
```

**Step 2: 在应用生命周期中管理服务**

在 `main.rs` 中注册 setup hook，启动 OCR 服务。

**Step 3: 验证编译并提交**

```bash
git add .
git commit -m "feat: add OCR service auto start/stop management"
```

---

## Phase 3: 发票解析模块

### Task 8: 实现发票文本解析器

**Objective:** 从 OCR 识别的文本中提取发票结构化数据。

**Files:**
- Create: `src-tauri/src/parser/invoice_parser.rs`
- Create: `src-tauri/src/parser/mod.rs`

**Step 1: 创建发票解析器**

`src-tauri/src/parser/invoice_parser.rs`:
```rust
use crate::models::invoice::{Invoice, InvoiceCategory, InvoiceSource, Itinerary};
use crate::ocr::client::OcrTextItem;
use regex::Regex;
use uuid::Uuid;

pub fn parse_invoice_text(
    texts: &[OcrTextItem],
    source: InvoiceSource,
) -> Result<Invoice, String> {
    let all_text: String = texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");

    // 提取金额
    let amount = extract_amount(&all_text)?;
    // 提取销售方名称
    let seller_name = extract_seller_name(&all_text);
    // 提取项目名称
    let item_name = extract_item_name(&all_text);
    // 提取日期
    let date = extract_date(&all_text);
    // 提取发票号码
    let invoice_number = extract_invoice_number(&all_text);
    // 智能分类
    let category = classify_invoice(&seller_name, &item_name);

    Ok(Invoice {
        id: Uuid::new_v4().to_string(),
        invoice_number,
        amount,
        seller_name,
        item_name,
        date,
        category,
        source,
        itineraries: vec![],
    })
}

fn extract_amount(text: &str) -> Result<f64, String> {
    // 匹配 "价税合计" "合计金额" "总金额" 等
    let re = Regex::new(r"(?:价税合计|合计金额|总金额|金额)[：:￥¥]?\s*([\d,]+\.?\d*)")
        .map_err(|e| e.to_string())?;
    if let Some(caps) = re.captures(text) {
        let amount_str = caps[1].replace(",", "");
        return amount_str.parse::<f64>().map_err(|e| e.to_string());
    }
    // 兜底：找最大的金额数字
    let re2 = Regex::new(r"￥\s*([\d,]+\.?\d*)").map_err(|e| e.to_string())?;
    let mut max_amount = 0.0f64;
    for cap in re2.captures_iter(text) {
        let v: f64 = cap[1].replace(",", "").parse().unwrap_or(0.0);
        if v > max_amount { max_amount = v; }
    }
    if max_amount > 0.0 { return Ok(max_amount); }
    Err("无法识别发票金额".to_string())
}

fn extract_seller_name(text: &str) -> String {
    let re = Regex::new(r"(?:销售方|收款单位|开票方)[：:]\s*(.+?)(?:\s|$)").unwrap();
    if let Some(caps) = re.captures(text) {
        return caps[1].trim().to_string();
    }
    String::new()
}

fn extract_item_name(text: &str) -> String {
    let re = Regex::new(r"(?:项目名称|货物或应税劳务|商品名称)[：:]\s*(.+?)(?:\s|$)").unwrap();
    if let Some(caps) = re.captures(text) {
        return caps[1].trim().to_string();
    }
    String::new()
}

fn extract_date(text: &str) -> chrono::NaiveDate {
    let re = Regex::new(r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日").unwrap();
    if let Some(caps) = re.captures(text) {
        let y: i32 = caps[1].parse().unwrap_or(2025);
        let m: u32 = caps[2].parse().unwrap_or(1);
        let d: u32 = caps[3].parse().unwrap_or(1);
        return chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default();
    }
    let re2 = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap();
    if let Some(caps) = re2.captures(text) {
        let y: i32 = caps[1].parse().unwrap_or(2025);
        let m: u32 = caps[2].parse().unwrap_or(1);
        let d: u32 = caps[3].parse().unwrap_or(1);
        return chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default();
    }
    chrono::NaiveDate::default()
}

fn extract_invoice_number(text: &str) -> String {
    let re = Regex::new(r"(?:发票号码|No)[：:]\s*(\d+)").unwrap();
    if let Some(caps) = re.captures(text) {
        return caps[1].to_string();
    }
    String::new()
}

pub fn classify_invoice(seller_name: &str, item_name: &str) -> InvoiceCategory {
    let combined = format!("{} {}", seller_name, item_name).to_lowercase();
    let combined = combined.as_str();

    if contains_any(combined, &["铁路", "高铁", "火车", "客运站"]) {
        InvoiceCategory::Train
    } else if contains_any(combined, &["航空", "机票", "机场", "航班"]) {
        InvoiceCategory::Flight
    } else if contains_any(combined, &["退票", "改签", "保险"]) {
        InvoiceCategory::TicketChange
    } else if contains_any(combined, &["出租", "网约车", "滴滴", "高德", "t3", "曹操"]) {
        InvoiceCategory::CityTransport
    } else if contains_any(combined, &["酒店", "宾馆", "住宿", "招待所", "民宿"]) {
        InvoiceCategory::Hotel
    } else if contains_any(combined, &["餐饮", "饭店", "食品", "餐厅", "饭馆"]) {
        InvoiceCategory::Meal
    } else {
        InvoiceCategory::Other
    }
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|k| text.contains(k))
}
```

`src-tauri/src/parser/mod.rs`:
```rust
pub mod invoice_parser;
pub use invoice_parser::parse_invoice_text;
pub use invoice_parser::classify_invoice;
```

**Step 2: 添加 uuid, regex 依赖到 Cargo.toml**

```toml
uuid = { version = "1", features = ["v4"] }
regex = "1"
chrono = { version = "0.4", features = ["serde"] }
```

**Step 3: 验证编译并提交**

```bash
git add .
git commit -m "feat: implement invoice text parser with regex extraction"
```

---

### Task 9: 实现行程单解析器

**Objective:** 解析打车行程单 PDF，提取每段行程信息。

**Files:**
- Create: `src-tauri/src/parser/itinerary_parser.rs`
- Modify: `src-tauri/src/parser/mod.rs`

**Step 1: 创建行程单解析器**

`src-tauri/src/parser/itinerary_parser.rs`:
```rust
use crate::models::invoice::Itinerary;
use crate::ocr::client::OcrTextItem;
use regex::Regex;

pub fn parse_itinerary_text(texts: &[OcrTextItem]) -> Vec<Itinerary> {
    let all_text: String = texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join("\n");

    // 尝试按行程段落解析
    let mut itineraries = Vec::new();

    // 常见行程单格式：
    // 2025-08-05 09:30  滴滴出行  ¥35.00
    // 或分段格式
    let re = Regex::new(
        r"(?m)(\d{4}[-/]\d{2}[-/]\d{2}\s+\d{2}:\d{2})\s+(.+?)\s+[¥￥]\s*([\d.]+)"
    ).unwrap();

    for cap in re.captures_iter(&all_text) {
        itineraries.push(Itinerary {
            date_time: cap[1].to_string(),
            provider: cap[2].trim().to_string(),
            pickup: String::new(),
            dropoff: String::new(),
            amount: cap[3].parse().unwrap_or(0.0),
        });
    }

    // 如果无法匹配标准格式，尝试其他格式
    if itineraries.is_empty() {
        itineraries = parse_fallback_format(&all_text);
    }

    itineraries
}

fn parse_fallback_format(text: &str) -> Vec<Itinerary> {
    let mut results = Vec::new();
    // 简单按行扫描，查找金额模式
    let re_amount = Regex::new(r"[¥￥]\s*([\d.]+)").unwrap();
    let re_time = Regex::new(r"(\d{2}:\d{2})").unwrap();

    for line in text.lines() {
        if let Some(amt) = re_amount.captures(line) {
            let amount: f64 = amt[1].parse().unwrap_or(0.0);
            if amount > 0.0 {
                let time = re_time.captures(line)
                    .map(|c| c[1].to_string())
                    .unwrap_or_default();
                results.push(Itinerary {
                    date_time: time,
                    provider: String::new(),
                    pickup: String::new(),
                    dropoff: String::new(),
                    amount,
                });
            }
        }
    }
    results
}
```

**Step 2: 注册模块并验证编译**

```rust
// parser/mod.rs 添加
pub mod itinerary_parser;
pub use itinerary_parser::parse_itinerary_text;
```

```bash
git add .
git commit -m "feat: implement itinerary parser for ride-hailing PDFs"
```

---

### Task 10: 实现发票去重逻辑

**Objective:** 根据发票号码去重，避免重复录入。

**Files:**
- Create: `src-tauri/src/parser/dedup.rs`
- Modify: `src-tauri/src/parser/mod.rs`

**Step 1: 创建去重模块**

`src-tauri/src/parser/dedup.rs`:
```rust
use crate::models::invoice::Invoice;
use std::collections::HashSet;

pub fn deduplicate_invoices(invoices: &mut Vec<Invoice>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();
    let mut unique = Vec::new();

    for invoice in invoices.drain(..) {
        if invoice.invoice_number.is_empty() {
            // 无发票号码，按金额+日期+销售方去重
            let key = format!("{}_{}_{}", invoice.amount, invoice.date, invoice.seller_name);
            if seen.insert(key) {
                unique.push(invoice);
            } else {
                duplicates.push(invoice.invoice_number.clone());
            }
        } else if seen.insert(invoice.invoice_number.clone()) {
            unique.push(invoice);
        } else {
            duplicates.push(invoice.invoice_number.clone());
        }
    }

    *invoices = unique;
    duplicates
}
```

**Step 2: 注册模块并提交**

```bash
git add .
git commit -m "feat: add invoice deduplication by invoice number"
```

---

### Task 11: 注册 Tauri 命令 — 发票识别与解析

**Objective:** 前端可调用发票识别与解析全流程。

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: 添加 Tauri 命令**

```rust
use crate::models::invoice::{Invoice, InvoiceSource};
use crate::parser::{parse_invoice_text, parse_itinerary_text, dedup::deduplicate_invoices};

#[tauri::command]
async fn recognize_invoice(
    state: tauri::State<'_, AppState>,
    file_path: String,
    file_type: String,  // "image" | "pdf"
) -> Result<Invoice, String> {
    let client = state.ocr_client.lock().map_err(|e| e.to_string())?;

    let source = InvoiceSource::Pdf(file_path.clone());

    let result = if file_type == "pdf" {
        let resp = client.recognize_pdf(&file_path).await?;
        let all_texts: Vec<OcrTextItem> = resp.pages.iter()
            .flat_map(|p| p.texts.clone())
            .collect();
        parse_invoice_text(&all_texts, source)?
    } else {
        let resp = client.recognize_image(&file_path).await?;
        parse_invoice_text(&resp.texts, source)?
    };

    Ok(result)
}

#[tauri::command]
async fn recognize_itinerary(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<Vec<Itinerary>, String> {
    let client = state.ocr_client.lock().map_err(|e| e.to_string())?;
    let resp = client.recognize_pdf(&file_path).await?;
    let all_texts: Vec<OcrTextItem> = resp.pages.iter()
        .flat_map(|p| p.texts.clone())
        .collect();
    Ok(parse_itinerary_text(&all_texts))
}
```

**Step 2: 注册命令并提交**

```bash
git add .
git commit -m "feat: register Tauri commands for invoice and itinerary recognition"
```

---

### Task 12: 实现发票链接/二维码解析

**Objective:** 支持从发票链接获取发票信息。

**Files:**
- Create: `src-tauri/src/parser/link_parser.rs`
- Modify: `src-tauri/src/parser/mod.rs`

**Step 1: 创建链接解析器**

`src-tauri/src/parser/link_parser.rs`:
```rust
/// 解析发票链接，提取发票信息
/// 支持的链接格式：
/// - 全国增值税发票查验平台链接
/// - 电子发票短链接
/// - 二维码中的链接
use reqwest::Client;

pub async fn fetch_invoice_from_link(url: &str) -> Result<String, String> {
    let client = Client::new();
    let resp = client.get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let html = resp.text().await.map_err(|e| e.to_string())?;
    Ok(html)
}

/// 从二维码图片中提取发票链接
pub fn extract_url_from_qrcode(image_path: &str) -> Result<String, String> {
    // 使用 Rust qrcode 解码库
    // 或调用 Python 服务解码
    // 此处先用占位实现
    Err("二维码解析功能待实现".to_string())
}
```

**Step 2: 提交**

```bash
git add .
git commit -m "feat: add invoice link/QR code parser (stub)"
```

---

## Phase 4: 支付账单解析

### Task 13: 实现微信账单解析器

**Objective:** 解析微信导出的 CSV/Excel 账单。

**Files:**
- Create: `src-tauri/src/parser/wechat_parser.rs`
- Modify: `src-tauri/src/parser/mod.rs`

**Step 1: 创建微信账单解析器**

`src-tauri/src/parser/wechat_parser.rs`:
```rust
use crate::models::payment::{PaymentRecord, PaymentSource};
use calamine::{Reader, Xls, open_workbook};
use uuid::Uuid;

pub fn parse_wechat_bill(file_path: &str) -> Result<Vec<PaymentRecord>, String> {
    let mut workbook: Xls<_> = open_workbook(file_path)
        .map_err(|e| format!("打开微信账单失败: {}", e))?;

    let sheet = workbook.sheet_names().get(0)
        .ok_or("无工作表")?
        .clone();

    let range = workbook.worksheet_range(&sheet)
        .map_err(|e| format!("读取工作表失败: {}", e))?;

    let mut records = Vec::new();
    let mut header_found = false;

    for row in range.rows() {
        let first_cell = row.get(0)
            .and_then(|c| c.to_string().ok())
            .unwrap_or_default();

        // 微信账单前几行是标题，找到 "交易时间" 那行开始读取
        if first_cell.contains("交易时间") {
            header_found = true;
            continue;
        }

        if !header_found { continue; }

        // 列顺序: 交易时间, 交易类型, 交易对方, 商品, 收/支, 金额(元), 支付方式, 当前状态, 交易单号, 商户单号, 备注
        if row.len() < 9 { continue; }

        let transaction_time = row.get(0).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let _trade_type = row.get(1).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let merchant_name = row.get(2).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let category = row.get(3).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let direction = row.get(4).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let amount_str = row.get(5).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let transaction_id = row.get(8).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();

        // 只取支出记录
        if !direction.contains("支") { continue; }

        let amount: f64 = amount_str
            .replace("¥", "")
            .replace(",", "")
            .trim()
            .parse()
            .unwrap_or(0.0);

        if amount <= 0.0 { continue; }

        records.push(PaymentRecord {
            id: Uuid::new_v4().to_string(),
            transaction_id,
            transaction_time,
            amount,
            merchant_name,
            source: PaymentSource::Wechat,
            category,
        });
    }

    Ok(records)
}
```

**Step 2: 添加 calamine 依赖并验证**

```toml
calamine = "0.26"
```

```bash
git add .
git commit -m "feat: implement WeChat bill parser"
```

---

### Task 14: 实现支付宝账单解析器

**Objective:** 解析支付宝导出的 CSV/Excel 账单。

**Files:**
- Create: `src-tauri/src/parser/alipay_parser.rs`
- Modify: `src-tauri/src/parser/mod.rs`

**Step 1: 创建支付宝账单解析器**

`src-tauri/src/parser/alipay_parser.rs`:
```rust
use crate::models::payment::{PaymentRecord, PaymentSource};
use calamine::{Reader, Xls, open_workbook};
use uuid::Uuid;

pub fn parse_alipay_bill(file_path: &str) -> Result<Vec<PaymentRecord>, String> {
    let mut workbook: Xls<_> = open_workbook(file_path)
        .map_err(|e| format!("打开支付宝账单失败: {}", e))?;

    let sheet = workbook.sheet_names().get(0)
        .ok_or("无工作表")?
        .clone();

    let range = workbook.worksheet_range(&sheet)
        .map_err(|e| format!("读取工作表失败: {}", e))?;

    let mut records = Vec::new();
    let mut header_found = false;

    for row in range.rows() {
        let first_cell = row.get(0)
            .and_then(|c| c.to_string().ok())
            .unwrap_or_default();

        // 支付宝账单标题行
        if first_cell.contains("交易时间") || first_cell.contains("交易号") {
            header_found = true;
            continue;
        }

        if !header_found { continue; }

        if row.len() < 8 { continue; }

        let transaction_time = row.get(0).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let transaction_id = row.get(1).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let merchant_name = row.get(4).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let category = row.get(6).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let amount_str = row.get(8).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();
        let direction = row.get(9).map(|c| c.to_string().unwrap_or_default()).unwrap_or_default();

        if !direction.contains("支出") { continue; }

        let amount: f64 = amount_str
            .replace("¥", "")
            .replace(",", "")
            .trim()
            .parse()
            .unwrap_or(0.0);

        if amount <= 0.0 { continue; }

        records.push(PaymentRecord {
            id: Uuid::new_v4().to_string(),
            transaction_id,
            transaction_time,
            amount,
            merchant_name,
            source: PaymentSource::Alipay,
            category,
        });
    }

    Ok(records)
}
```

**Step 2: 提交**

```bash
git add .
git commit -m "feat: implement Alipay bill parser"
```

---

### Task 15: 注册 Tauri 命令 — 账单导入

**Objective:** 前端可调用账单解析功能。

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: 添加命令**

```rust
use crate::parser::{wechat_parser, alipay_parser};

#[tauri::command]
async fn import_wechat_bill(file_path: String) -> Result<Vec<PaymentRecord>, String> {
    wechat_parser::parse_wechat_bill(&file_path)
}

#[tauri::command]
async fn import_alipay_bill(file_path: String) -> Result<Vec<PaymentRecord>, String> {
    alipay_parser::parse_alipay_bill(&file_path)
}
```

**Step 2: 注册命令并提交**

```bash
git add .
git commit -m "feat: register Tauri commands for bill import"
```

---

## Phase 5: 匹配引擎

### Task 16: 实现金额匹配算法

**Objective:** 实现发票金额与支付金额的匹配（允许误差、一对多）。

**Files:**
- Create: `src-tauri/src/matching/engine.rs`
- Create: `src-tauri/src/matching/mod.rs`

**Step 1: 创建匹配引擎**

`src-tauri/src/matching/engine.rs`:
```rust
use crate::models::invoice::Invoice;
use crate::models::payment::PaymentRecord;
use crate::models::match_result::{MatchResult, MatchType};

const DEFAULT_TOLERANCE: f64 = 1.00;

pub struct MatchEngine {
    tolerance: f64,
}

impl MatchEngine {
    pub fn new(tolerance: f64) -> Self {
        Self { tolerance }
    }

    pub fn with_default_tolerance() -> Self {
        Self { tolerance: DEFAULT_TOLERANCE }
    }

    /// 1对1匹配：一张发票匹配一笔支付
    pub fn match_one_to_one(
        &self,
        invoice: &Invoice,
        payments: &[PaymentRecord],
    ) -> Option<MatchResult> {
        for payment in payments {
            let diff = (invoice.amount - payment.amount).abs();
            if diff <= self.tolerance {
                return Some(MatchResult {
                    invoice_id: invoice.id.clone(),
                    invoice: invoice.clone(),
                    payment_ids: vec![payment.id.clone()],
                    payments: vec![payment.clone()],
                    match_type: MatchType::OneToOne,
                    confidence: 1.0 - (diff / invoice.amount.max(0.01)),
                    amount_diff: diff,
                });
            }
        }
        None
    }

    /// 1对多匹配：一张发票匹配多笔支付（打车场景）
    pub fn match_one_to_many(
        &self,
        invoice: &Invoice,
        payments: &[PaymentRecord],
    ) -> Option<MatchResult> {
        // 尝试子集和匹配：找到支付子集，其总金额在误差范围内
        let matched = self.subset_sum_match(invoice.amount, payments, self.tolerance)?;

        let total: f64 = matched.iter().map(|p| p.amount).sum();
        let diff = (invoice.amount - total).abs();

        Some(MatchResult {
            invoice_id: invoice.id.clone(),
            invoice: invoice.clone(),
            payment_ids: matched.iter().map(|p| p.id.clone()).collect(),
            payments: matched,
            match_type: MatchType::OneToMany,
            confidence: 1.0 - (diff / invoice.amount.max(0.01)),
            amount_diff: diff,
        })
    }

    /// 子集和匹配：找支付子集，使其总金额接近目标
    fn subset_sum_match(
        &self,
        target: f64,
        payments: &[PaymentRecord],
        tolerance: f64,
    ) -> Option<Vec<PaymentRecord>> {
        // 限制搜索范围，避免组合爆炸
        let max_subset_size = 10;
        let n = payments.len().min(20); // 最多考虑20笔

        // 递归搜索
        self.search_subset(target, tolerance, &payments[..n], 0, max_subset_size, vec![])
    }

    fn search_subset(
        &self,
        target: f64,
        tolerance: f64,
        payments: &[PaymentRecord],
        start: usize,
        remaining: usize,
        current: Vec<PaymentRecord>,
    ) -> Option<Vec<PaymentRecord>> {
        let current_sum: f64 = current.iter().map(|p| p.amount).sum();

        if (current_sum - target).abs() <= tolerance && !current.is_empty() {
            return Some(current);
        }

        if remaining == 0 || start >= payments.len() || current_sum > target + tolerance {
            return None;
        }

        for i in start..payments.len() {
            let mut next = current.clone();
            next.push(payments[i].clone());
            if let Some(result) = self.search_subset(target, tolerance, payments, i + 1, remaining - 1, next) {
                return Some(result);
            }
        }

        None
    }
}
```

`src-tauri/src/matching/mod.rs`:
```rust
pub mod engine;
pub use engine::MatchEngine;
```

**Step 2: 提交**

```bash
git add .
git commit -m "feat: implement matching engine with one-to-one and one-to-many"
```

---

### Task 17: 实现批量匹配流程

**Objective:** 对所有发票执行自动匹配，返回匹配结果和未匹配列表。

**Files:**
- Create: `src-tauri/src/matching/batch.rs`
- Modify: `src-tauri/src/matching/mod.rs`

**Step 1: 创建批量匹配**

`src-tauri/src/matching/batch.rs`:
```rust
use crate::models::invoice::{Invoice, InvoiceCategory};
use crate::models::payment::PaymentRecord;
use crate::models::match_result::{MatchResult, MatchType};
use super::engine::MatchEngine;

pub struct BatchMatchResult {
    pub matched: Vec<MatchResult>,
    pub unmatched_invoices: Vec<Invoice>,
    pub unmatched_payments: Vec<PaymentRecord>,
}

pub fn batch_match(
    invoices: &[Invoice],
    payments: &[PaymentRecord],
    tolerance: f64,
) -> BatchMatchResult {
    let engine = MatchEngine::new(tolerance);
    let mut matched = Vec::new();
    let mut unmatched_invoices = Vec::new();
    let mut used_payment_ids: Vec<String> = Vec::new();

    for invoice in invoices {
        let available_payments: Vec<PaymentRecord> = payments.iter()
            .filter(|p| !used_payment_ids.contains(&p.id))
            .cloned()
            .collect();

        let result = if invoice.category == InvoiceCategory::CityTransport && !invoice.itineraries.is_empty() {
            // 打车场景：一对多匹配
            engine.match_one_to_many(invoice, &available_payments)
        } else {
            // 普通场景：一对一匹配
            engine.match_one_to_one(invoice, &available_payments)
        };

        if let Some(match_result) = result {
            for pid in &match_result.payment_ids {
                used_payment_ids.push(pid.clone());
            }
            matched.push(match_result);
        } else {
            unmatched_invoices.push(invoice.clone());
        }
    }

    let unmatched_payments: Vec<PaymentRecord> = payments.iter()
        .filter(|p| !used_payment_ids.contains(&p.id))
        .cloned()
        .collect();

    BatchMatchResult {
        matched,
        unmatched_invoices,
        unmatched_payments,
    }
}
```

**Step 2: 提交**

```bash
git add .
git commit -m "feat: implement batch matching with city-transport one-to-many"
```

---

### Task 18: 实现手动匹配与调整

**Objective:** 支持用户手动修改匹配结果。

**Files:**
- Create: `src-tauri/src/matching/manual.rs`
- Modify: `src-tauri/src/matching/mod.rs`

**Step 1: 创建手动匹配 API**

`src-tauri/src/matching/manual.rs`:
```rust
use crate::models::invoice::Invoice;
use crate::models::payment::PaymentRecord;
use crate::models::match_result::{MatchResult, MatchType};

/// 手动创建匹配
pub fn create_manual_match(
    invoice: Invoice,
    payments: Vec<PaymentRecord>,
) -> MatchResult {
    let total: f64 = payments.iter().map(|p| p.amount).sum();
    let diff = (invoice.amount - total).abs();

    MatchResult {
        invoice_id: invoice.id.clone(),
        invoice,
        payment_ids: payments.iter().map(|p| p.id.clone()).collect(),
        payments,
        match_type: MatchType::ManualConfirmed,
        confidence: if diff == 0.0 { 1.0 } else { 0.8 },
        amount_diff: diff,
    }
}

/// 取消匹配，释放支付记录
pub fn unmatch_invoice(
    match_result: &MatchResult,
) -> (Invoice, Vec<PaymentRecord>) {
    (match_result.invoice.clone(), match_result.payments.clone())
}
```

**Step 2: 提交**

```bash
git add .
git commit -m "feat: add manual match and unmatch operations"
```

---

### Task 19: 注册 Tauri 命令 — 匹配引擎

**Objective:** 前端可调用匹配功能。

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: 添加命令**

```rust
use crate::matching::{engine, batch, manual};
use crate::models::match_result::MatchResult;

#[tauri::command]
async fn auto_match(
    invoices: Vec<Invoice>,
    payments: Vec<PaymentRecord>,
    tolerance: f64,
) -> Result<serde_json::Value, String> {
    let result = batch::batch_match(&invoices, &payments, tolerance);
    Ok(serde_json::to_value(result).map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn manual_match(
    invoice: Invoice,
    payments: Vec<PaymentRecord>,
) -> Result<MatchResult, String> {
    Ok(manual::create_manual_match(invoice, payments))
}
```

**Step 2: 注册命令并提交**

```bash
git add .
git commit -m "feat: register Tauri commands for matching engine"
```

---

## Phase 6: PDF 生成模块

### Task 20: 实现报销表单 PDF 生成

**Objective:** 根据匹配结果生成报销汇总 PDF 表单。

**Files:**
- Create: `src-tauri/src/pdf/form_generator.rs`
- Create: `src-tauri/src/pdf/mod.rs`

**Step 1: 创建 PDF 表单生成器**

`src-tauri/src/pdf/form_generator.rs`:
```rust
use crate::models::reimbursement::{ReimbursementForm, CategorySummary};
use crate::models::invoice::InvoiceCategory;
use genpdf::{Document, elements, fonts};
use std::error::Error;

// 中文类别标签
fn category_label(cat: &InvoiceCategory) -> &str {
    match cat {
        InvoiceCategory::Train => "车、船票",
        InvoiceCategory::Flight => "飞机票",
        InvoiceCategory::TicketChange => "订（退、改签）票及交通保险费",
        InvoiceCategory::CityTransport => "市内交通费",
        InvoiceCategory::Hotel => "住宿费",
        InvoiceCategory::Meal => "餐补/伙食补助",
        InvoiceCategory::Other => "其他",
    }
}

pub fn generate_reimbursement_pdf(
    form: &ReimbursementForm,
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let font_dir = "/usr/share/fonts/truetype/noto/";
    let regular = fonts::Font::from_file(format!("{}NotoSansCJK-Regular.ttc", font_dir), 0)?;
    let bold = fonts::Font::from_file(format!("{}NotoSansCJK-Bold.ttc", font_dir), 0)?;
    let font_family = fonts::FontFamily {
        regular,
        bold,
        italic: regular.clone(),
        bold_italic: bold.clone(),
    };

    let mut doc = Document::new(font_family);
    doc.set_title("差旅费报销表");

    // 标题
    doc.push(elements::Paragraph::new("差旅费报销表").aligned(genpdf::Alignment::Center));

    // 基本信息
    doc.push(elements::Paragraph::new(format!(
        "姓名：{}  部职别：{}", form.name, form.department
    )));
    doc.push(elements::Paragraph::new(format!(
        "出差日期：{} 至 {}  同行人数：{}", form.travel_start, form.travel_end, form.companions
    )));

    // 城市间交通费
    doc.push(elements::Paragraph::new("城市间交通费"));
    for s in &form.summaries {
        if matches!(s.category, InvoiceCategory::Train | InvoiceCategory::Flight | InvoiceCategory::TicketChange) {
            doc.push(elements::Paragraph::new(format!(
                "  {}  单据张数：{}  申报金额：{:.2}", category_label(&s.category), s.count, s.total_amount
            )));
        }
    }

    // 市内交通费
    doc.push(elements::Paragraph::new("市内交通费"));
    if let Some(s) = form.summaries.iter().find(|s| matches!(s.category, InvoiceCategory::CityTransport)) {
        doc.push(elements::Paragraph::new(format!(
            "  单据张数：{}  申报金额：{:.2}", s.count, s.total_amount
        )));
    }

    // 住宿费
    doc.push(elements::Paragraph::new("住宿费"));
    if let Some(s) = form.summaries.iter().find(|s| matches!(s.category, InvoiceCategory::Hotel)) {
        doc.push(elements::Paragraph::new(format!(
            "  单据张数：{}  申报金额：{:.2}", s.count, s.total_amount
        )));
    }

    // 总计
    doc.push(elements::Paragraph::new(format!(
        "申报金额总计：￥{:.2}", form.total_amount
    )));

    doc.render_to_file(output_path)?;
    Ok(())
}
```

`src-tauri/src/pdf/mod.rs`:
```rust
pub mod form_generator;
pub use form_generator::generate_reimbursement_pdf;
```

**Step 2: 添加依赖**

```toml
genpdf = "0.2"
printpdf = "0.7"
```

**Step 3: 提交**

```bash
git add .
git commit -m "feat: implement reimbursement form PDF generator"
```

---

### Task 21: 实现发票-支付对照 PDF 生成（普通发票）

**Objective:** 生成发票图片+支付记录对照页。

**Files:**
- Create: `src-tauri/src/pdf/evidence_generator.rs`
- Modify: `src-tauri/src/pdf/mod.rs`

**Step 1: 创建对照 PDF 生成器**

`src-tauri/src/pdf/evidence_generator.rs`:
```rust
use crate::models::match_result::MatchResult;
use crate::models::invoice::InvoiceCategory;
use printpdf::*;
use std::io::BufWriter;
use std::fs::File;

/// 生成发票-支付对照 PDF
pub fn generate_evidence_pdf(
    match_results: &[MatchResult],
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (doc, page1, layer1) = PdfDocument::new(
        "发票支付对照",
        Mm(210.0),
        Mm(297.0),
        "Page 1",
    );
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let font = doc.add_external_font(File::open("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc")?)?;
    let font_size = 12.0;

    let mut y_pos = 280.0;

    for result in match_results {
        // 发票信息
        current_layer.use_text(
            format!("发票号码：{}  金额：¥{:.2}", result.invoice.invoice_number, result.invoice.amount),
            font_size,
            Mm(20.0),
            Mm(y_pos),
            &font,
        );
        y_pos -= 8.0;

        // 类别标签
        current_layer.use_text(
            format!("类别：{:?}", result.invoice.category),
            font_size,
            Mm(20.0),
            Mm(y_pos),
            &font,
        );
        y_pos -= 8.0;

        // 发票图片（如果有）
        // TODO: 嵌入发票图片

        // 支付记录
        for payment in &result.payments {
            current_layer.use_text(
                format!("支付单号：{}  金额：¥{:.2}  时间：{}", payment.transaction_id, payment.amount, payment.transaction_time),
                font_size,
                Mm(20.0),
                Mm(y_pos),
                &font,
            );
            y_pos -= 8.0;
        }

        y_pos -= 10.0;

        // 如果空间不足，添加新页
        if y_pos < 30.0 {
            // TODO: 添加新页并重置 y_pos
            y_pos = 280.0;
        }
    }

    doc.save(&mut BufWriter::new(File::create(output_path)?))?;
    Ok(())
}
```

**Step 2: 提交**

```bash
git add .
git commit -m "feat: implement evidence PDF generator for invoice-payment mapping"
```

---

### Task 22: 实现打车行程对照表 PDF 生成

**Objective:** 生成打车发票 + 行程单 + 支付对照表的特殊页面。

**Files:**
- Modify: `src-tauri/src/pdf/evidence_generator.rs`

**Step 1: 添加打车场景特殊渲染**

在 `evidence_generator.rs` 中添加:

```rust
/// 生成打车发票-行程单-支付对照页面
fn render_taxi_evidence(
    doc: &PdfDocumentReference,
    result: &MatchResult,
    current_layer: &PdfLayerReference,
    font: &IndirectFontRef,
    y_pos: &mut f64,
) {
    // 第1部分：发票信息
    current_layer.use_text(
        format!("【打车发票】号码：{}  金额：¥{:.2}", result.invoice.invoice_number, result.invoice.amount),
        14.0,
        Mm(20.0),
        Mm(*y_pos),
        font,
    );
    *y_pos -= 10.0;

    // 第2部分：行程单列表
    current_layer.use_text("【行程明细】", 12.0, Mm(20.0), Mm(*y_pos), font);
    *y_pos -= 8.0;

    for (i, it) in result.invoice.itineraries.iter().enumerate() {
        current_layer.use_text(
            format!("{}. {} {} {} → {} ¥{:.2}", i + 1, it.date_time, it.provider, it.pickup, it.dropoff, it.amount),
            10.0,
            Mm(25.0),
            Mm(*y_pos),
            font,
        );
        *y_pos -= 7.0;
    }

    *y_pos -= 5.0;

    // 第3部分：行程-支付对照表
    current_layer.use_text("【行程-支付对照表】", 12.0, Mm(20.0), Mm(*y_pos), font);
    *y_pos -= 8.0;

    // 表头
    current_layer.use_text("时间         金额       支付单号", 10.0, Mm(25.0), Mm(*y_pos), font);
    *y_pos -= 7.0;

    for (it, pay) in result.invoice.itineraries.iter().zip(result.payments.iter()) {
        current_layer.use_text(
            format!("{}  ¥{:.2}  {}", it.date_time, it.amount, pay.transaction_id),
            10.0,
            Mm(25.0),
            Mm(*y_pos),
            font,
        );
        *y_pos -= 7.0;
    }

    // 合计行
    let total: f64 = result.invoice.itineraries.iter().map(|i| i.amount).sum();
    current_layer.use_text(
        format!("合计  ¥{:.2}", total),
        11.0,
        Mm(25.0),
        Mm(*y_pos),
        font,
    );
}
```

**Step 2: 提交**

```bash
git add .
git commit -m "feat: add taxi itinerary-payment mapping table in evidence PDF"
```

---

### Task 23: 实现报销汇总计算器

**Objective:** 根据匹配结果自动计算报销汇总。

**Files:**
- Create: `src-tauri/src/matching/summary.rs`
- Modify: `src-tauri/src/matching/mod.rs`

**Step 1: 创建汇总计算器**

`src-tauri/src/matching/summary.rs`:
```rust
use crate::models::invoice::InvoiceCategory;
use crate::models::match_result::MatchResult;
use crate::models::reimbursement::{ReimbursementForm, CategorySummary};
use std::collections::HashMap;

pub fn calculate_summary(
    match_results: &[MatchResult],
    name: &str,
    department: &str,
    travel_start: &str,
    travel_end: &str,
    companions: usize,
) -> ReimbursementForm {
    let mut category_map: HashMap<InvoiceCategory, (usize, f64)> = HashMap::new();

    for result in match_results {
        let entry = category_map
            .entry(result.invoice.category.clone())
            .or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += result.invoice.amount;
    }

    let summaries: Vec<CategorySummary> = category_map
        .into_iter()
        .map(|(cat, (count, total))| CategorySummary {
            category: cat,
            count,
            total_amount: total,
        })
        .collect();

    let total_amount: f64 = summaries.iter().map(|s| s.total_amount).sum();

    ReimbursementForm {
        name: name.to_string(),
        department: department.to_string(),
        travel_start: travel_start.to_string(),
        travel_end: travel_end.to_string(),
        companions,
        summaries,
        total_amount,
    }
}
```

**Step 2: 提交**

```bash
git add .
git commit -m "feat: add reimbursement summary calculator"
```

---

### Task 24: 注册 Tauri 命令 — PDF 生成

**Objective:** 前端可调用 PDF 生成功能。

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: 添加命令**

```rust
use crate::pdf::{form_generator, evidence_generator};
use crate::matching::summary;

#[tauri::command]
async fn generate_form_pdf(
    form: ReimbursementForm,
    output_path: String,
) -> Result<(), String> {
    form_generator::generate_reimbursement_pdf(&form, &output_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn generate_evidence_pdf(
    match_results: Vec<MatchResult>,
    output_path: String,
) -> Result<(), String> {
    evidence_generator::generate_evidence_pdf(&match_results, &output_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn calculate_reimbursement(
    match_results: Vec<MatchResult>,
    name: String,
    department: String,
    travel_start: String,
    travel_end: String,
    companions: usize,
) -> Result<ReimbursementForm, String> {
    Ok(summary::calculate_summary(
        &match_results, &name, &department, &travel_start, &travel_end, companions
    ))
}
```

**Step 2: 注册命令并提交**

```bash
git add .
git commit -m "feat: register Tauri commands for PDF generation and summary"
```

---

## Phase 7: 前端界面

### Task 25: 搭建前端布局与路由

**Objective:** 创建应用主布局和页面路由。

**Files:**
- Create: `src/views/HomeView.vue`
- Create: `src/views/ImportView.vue`
- Create: `src/views/MatchView.vue`
- Create: `src/views/ExportView.vue`
- Create: `src/router/index.ts`
- Modify: `src/App.vue`

**Step 1: 创建路由**

`src/router/index.ts`:
```ts
import { createRouter, createWebHashHistory } from 'vue-router'

const routes = [
  { path: '/', name: 'home', component: () => import('../views/HomeView.vue') },
  { path: '/import', name: 'import', component: () => import('../views/ImportView.vue') },
  { path: '/match', name: 'match', component: () => import('../views/MatchView.vue') },
  { path: '/export', name: 'export', component: () => import('../views/ExportView.vue') },
]

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
})
```

**Step 2: 创建布局**

`src/App.vue`:
```vue
<template>
  <div class="h-screen flex flex-col bg-gray-50">
    <nav class="flex items-center gap-4 px-6 py-3 bg-white border-b">
      <h1 class="text-lg font-bold">发票报销助手</h1>
      <router-link to="/" class="nav-link">首页</router-link>
      <router-link to="/import" class="nav-link">导入</router-link>
      <router-link to="/match" class="nav-link">匹配</router-link>
      <router-link to="/export" class="nav-link">导出</router-link>
    </nav>
    <main class="flex-1 overflow-auto p-6">
      <router-view />
    </main>
  </div>
</template>
```

**Step 3: 创建各页面占位组件**

`src/views/HomeView.vue`: 欢迎页 + OCR 服务状态
`src/views/ImportView.vue`: 发票 + 账单导入
`src/views/MatchView.vue`: 匹配结果展示
`src/views/ExportView.vue`: PDF 导出

**Step 4: 提交**

```bash
git add .
git commit -m "feat: setup frontend layout, router, and page stubs"
```

---

### Task 26: 实现发票导入页面

**Objective:** 拖拽上传发票文件，实时显示识别结果。

**Files:**
- Modify: `src/views/ImportView.vue`
- Create: `src/stores/invoice.ts`
- Create: `src/components/InvoiceDropZone.vue`
- Create: `src/components/InvoiceCard.vue`

**Step 1: 创建发票 Store**

`src/stores/invoice.ts`:
```ts
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Invoice } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const useInvoiceStore = defineStore('invoice', () => {
  const invoices = ref<Invoice[]>([])
  const loading = ref(false)

  async function addInvoice(filePath: string, fileType: string) {
    loading.value = true
    try {
      const invoice: Invoice = await invoke('recognize_invoice', { filePath, fileType })
      invoices.value.push(invoice)
    } finally {
      loading.value = false
    }
  }

  function removeInvoice(id: string) {
    invoices.value = invoices.value.filter(i => i.id !== id)
  }

  return { invoices, loading, addInvoice, removeInvoice }
})
```

**Step 2: 创建拖拽组件**

`src/components/InvoiceDropZone.vue`:
```vue
<template>
  <div
    class="border-2 border-dashed rounded-lg p-8 text-center cursor-pointer
           hover:border-blue-400 hover:bg-blue-50 transition-colors"
    :class="isDragging ? 'border-blue-500 bg-blue-50' : 'border-gray-300'"
    @dragover.prevent="isDragging = true"
    @dragleave="isDragging = false"
    @drop.prevent="handleDrop"
    @click="openFilePicker"
  >
    <p class="text-gray-500">
      {{ isDragging ? '松开以上传' : '拖拽发票文件到此处，或点击选择' }}
    </p>
    <p class="text-sm text-gray-400 mt-2">支持 PDF / 图片 / 多文件</p>
  </div>
</template>
```

**Step 3: 创建发票卡片组件**

显示发票号码、金额、类别、来源、操作按钮。

**Step 4: 提交**

```bash
git add .
git commit -m "feat: implement invoice import page with drag-drop and OCR"
```

---

### Task 27: 实现账单导入页面

**Objective:** 上传微信/支付宝账单，展示解析结果。

**Files:**
- Create: `src/stores/payment.ts`
- Create: `src/components/BillImporter.vue`
- Create: `src/components/PaymentTable.vue`

**Step 1: 创建支付记录 Store**

`src/stores/payment.ts`:
```ts
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { PaymentRecord } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const usePaymentStore = defineStore('payment', () => {
  const payments = ref<PaymentRecord[]>([])

  async function importWechatBill(filePath: string) {
    const records: PaymentRecord[] = await invoke('import_wechat_bill', { filePath })
    payments.value.push(...records)
  }

  async function importAlipayBill(filePath: string) {
    const records: PaymentRecord[] = await invoke('import_alipay_bill', { filePath })
    payments.value.push(...records)
  }

  return { payments, importWechatBill, importAlipayBill }
})
```

**Step 2: 创建账单导入组件**

支持选择文件类型（微信/支付宝），上传后展示表格。

**Step 3: 提交**

```bash
git add .
git commit -m "feat: implement bill import page for WeChat and Alipay"
```

---

### Task 28: 实现匹配结果页面

**Objective:** 展示自动匹配结果，支持手动调整。

**Files:**
- Modify: `src/views/MatchView.vue`
- Create: `src/stores/match.ts`
- Create: `src/components/MatchCard.vue`
- Create: `src/components/MatchAdjustDialog.vue`

**Step 1: 创建匹配 Store**

`src/stores/match.ts`:
```ts
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { MatchResult, Invoice, PaymentRecord } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const useMatchStore = defineStore('match', () => {
  const matches = ref<MatchResult[]>([])
  const unmatchedInvoices = ref<Invoice[]>([])
  const unmatchedPayments = ref<PaymentRecord[]>([])

  async function autoMatch(invoices: Invoice[], payments: PaymentRecord[], tolerance = 1.0) {
    const result = await invoke('auto_match', { invoices, payments, tolerance })
    matches.value = result.matched
    unmatchedInvoices.value = result.unmatched_invoices
    unmatchedPayments.value = result.unmatched_payments
  }

  async function manualMatch(invoice: Invoice, payments: PaymentRecord[]) {
    const matchResult = await invoke('manual_match', { invoice, payments })
    matches.value.push(matchResult)
    unmatchedInvoices.value = unmatchedInvoices.value.filter(i => i.id !== invoice.id)
  }

  return { matches, unmatchedInvoices, unmatchedPayments, autoMatch, manualMatch }
})
```

**Step 2: 创建匹配卡片**

展示发票-支付对应关系，置信度颜色标记，手动调整按钮。

**Step 3: 提交**

```bash
git add .
git commit -m "feat: implement match result page with manual adjustment"
```

---

### Task 29: 实现导出页面

**Objective:** 填写报销信息，一键生成 PDF。

**Files:**
- Modify: `src/views/ExportView.vue`
- Create: `src/components/ReimbursementForm.vue`
- Create: `src/components/ExportButton.vue`

**Step 1: 创建报销信息表单**

输入姓名、部职别、出差日期、同行人数。

**Step 2: 创建导出按钮**

- 生成报销表单 PDF
- 生成发票-支付对照 PDF
- 生成 Excel 备份

**Step 3: 提交**

```bash
git add .
git commit -m "feat: implement export page with PDF generation"
```

---

### Task 30: 实现首页仪表盘

**Objective:** 展示整体状态和快速操作入口。

**Files:**
- Modify: `src/views/HomeView.vue`

**Step 1: 实现仪表盘**

- OCR 服务连接状态
- 已导入发票数 / 支付记录数
- 已匹配 / 未匹配数量
- 快速操作按钮

**Step 2: 提交**

```bash
git add .
git commit -m "feat: implement home dashboard with status and quick actions"
```

---

### Task 31: 实现发票类别颜色与图标

**Objective:** 为不同发票类别设计视觉标识。

**Files:**
- Create: `src/utils/category.ts`

**Step 1: 创建类别映射**

```ts
import type { InvoiceCategory } from '../types'

export const CATEGORY_CONFIG: Record<InvoiceCategory, {
  label: string
  color: string
  bgColor: string
  icon: string
}> = {
  Train: { label: '高铁/车船票', color: 'text-blue-700', bgColor: 'bg-blue-100', icon: '🚄' },
  Flight: { label: '飞机票', color: 'text-indigo-700', bgColor: 'bg-indigo-100', icon: '✈️' },
  TicketChange: { label: '退改签/保险费', color: 'text-yellow-700', bgColor: 'bg-yellow-100', icon: '🔄' },
  CityTransport: { label: '市内交通', color: 'text-green-700', bgColor: 'bg-green-100', icon: '🚕' },
  Hotel: { label: '住宿费', color: 'text-purple-700', bgColor: 'bg-purple-100', icon: '🏨' },
  Meal: { label: '餐饮费', color: 'text-orange-700', bgColor: 'bg-orange-100', icon: '🍽️' },
  Other: { label: '其他', color: 'text-gray-700', bgColor: 'bg-gray-100', icon: '📋' },
}
```

**Step 2: 提交**

```bash
git add .
git commit -m "feat: add category color/icon configuration"
```

---

### Task 32: 实现发票预览与缩略图

**Objective:** 在发票卡片中显示缩略图预览。

**Files:**
- Create: `src/components/InvoicePreview.vue`

**Step 1: 创建预览组件**

- 照片：直接显示缩略图
- PDF：渲染第一页缩略图
- 链接：显示链接图标

**Step 2: 提交**

```bash
git add .
git commit -m "feat: add invoice preview with thumbnails"
```

---

## Phase 8: 集成测试与打包

### Task 33: 编写 Rust 单元测试

**Objective:** 为核心模块添加单元测试。

**Files:**
- Create: `src-tauri/src/parser/invoice_parser_test.rs`
- Create: `src-tauri/src/matching/engine_test.rs`

**Step 1: 发票解析测试**

测试金额提取、分类识别、日期解析等。

**Step 2: 匹配引擎测试**

测试一对一匹配、一对多匹配、误差容忍。

**Step 3: 运行测试**

```bash
cd src-tauri && cargo test
```

Expected: 所有测试通过。

**Step 4: 提交**

```bash
git add .
git commit -m "test: add unit tests for invoice parser and matching engine"
```

---

### Task 34: 端到端集成测试

**Objective:** 使用示例发票和账单进行全流程测试。

**Files:**
- Create: `tests/fixtures/` (测试用发票图片、PDF、账单文件)
- Create: `tests/e2e_test.rs`

**Step 1: 准备测试数据**

- 示例高铁票 PDF
- 示例住宿发票 PDF
- 示例打车发票 + 行程单
- 示例微信/支付宝账单

**Step 2: 端到端流程测试**

导入 → OCR → 解析 → 匹配 → 生成 PDF

**Step 3: 提交**

```bash
git add .
git commit -m "test: add end-to-end integration tests"
```

---

### Task 35: 应用打包与分发

**Objective:** 配置 Tauri 打包，生成安装程序。

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Create: `src-tauri/icons/` (应用图标)

**Step 1: 配置 tauri.conf.json**

设置应用名称、窗口大小、打包选项。

**Step 2: 生成图标**

**Step 3: 打包**

```bash
npm run tauri build
```

Expected: 生成 .deb / .AppImage (Linux) 或 .msi (Windows)

**Step 4: 提交**

```bash
git add .
git commit -m "feat: configure Tauri packaging and app icons"
```

---

## 总结

| Phase | 任务数 | 关键交付物 |
|-------|--------|-----------|
| 1. 项目骨架 | 3 | Tauri + Vue 3 项目、数据模型 |
| 2. OCR 服务 | 4 | Python OCR 服务、Rust 客户端 |
| 3. 发票解析 | 5 | 发票/行程单解析、去重、链接解析 |
| 4. 支付解析 | 3 | 微信/支付宝账单解析 |
| 5. 匹配引擎 | 4 | 金额匹配、批量匹配、手动调整 |
| 6. PDF 生成 | 5 | 报销表单、对照 PDF、汇总计算 |
| 7. 前端界面 | 8 | 完整 UI（导入/匹配/导出） |
| 8. 测试打包 | 3 | 单元测试、E2E测试、安装包 |
| **总计** | **35** | |
