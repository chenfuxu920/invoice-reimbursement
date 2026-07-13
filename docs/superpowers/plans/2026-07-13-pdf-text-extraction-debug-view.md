# PDF 文字提取调试对比界面 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 新增 `/debug` 调试界面，选择 PDF 后以渲染页面图片为底图，叠加 pdfplumber/zpdf/OCR 三个引擎的文字坐标框，可勾选切换、可拖动对齐比较。

**架构：** 后端新增 `pdf/debug_extract.rs` 模块，统一三个引擎坐标到渲染图片像素空间，通过一个 Tauri command 返回 `{pages: [{image, width, height, pdfplumber, zpdf, ocr}]}`。前端新增 `DebugView.vue`，用绝对定位 div 叠加文字框在底图上，原生鼠标事件实现拖动。

**技术栈：** Rust（zpdf 渲染 + pdfplumber + OCR）、Vue 3 + Tailwind v4、Tauri 2.x

**规格：** `docs/superpowers/specs/2026-07-13-pdf-text-extraction-debug-view-design.md`

---

## 关键事实（已核实）

- **crate 名**：`invoice_reimbursement_lib`
- **OCR 渲染 DPI = 200**（`src-tauri/src/ocr/engine.rs:372`），OCR 坐标即 200DPI 图片像素空间
- **pdfplumber 坐标**：PDF 点（72DPI），`extract_raw_words_debug(path) -> Vec<(text, x0, top, x1, bottom, page)>`
- **zpdf 坐标**：PDF 点（72DPI），已翻转为 y-down 屏幕坐标，`extract_text_with_zpdf(path) -> Vec<OcrTextItem>`，box_coords = `{points:[{x,y}*4], box_score}`
- **渲染**：`render_pdf_to_rgb_images(path, dpi) -> Vec<RgbImage>`
- **base64 编码模式**：`lib.rs:482-501`（`base64::engine::general_purpose::STANDARD.encode` → `data:image/png;base64,...`）
- **测试 PDF**：`data/发票与行程单/滴滴电子发票A.pdf`，路径 helper：`concat!(env!("CARGO_MANIFEST_DIR"), "/../data")`
- **坐标缩放**（渲染图片 dpi=D）：
  - pdfplumber/zpdf（PDF点）→ 像素：`× (D/72)`
  - OCR（200DPI像素）→ 像素：`× (D/200)`
- **command 注册**：`lib.rs:583-609` 的 `invoke_handler` 列表
- **路由**：`src/router/index.ts`，hash mode
- **HomeView 入口**：`src/views/HomeView.vue:70-86` 快速操作 grid

## 文件结构

| 文件 | 职责 | 动作 |
|------|------|------|
| `src-tauri/src/pdf/debug_extract.rs` | 调试提取类型 + 核心函数（渲染+三引擎+坐标统一） | 创建 |
| `src-tauri/src/pdf/mod.rs` | 模块声明 | 修改（加 `pub mod debug_extract;`） |
| `src-tauri/src/lib.rs` | Tauri command 包装 + 注册 | 修改 |
| `src-tauri/tests/debug_extract_test.rs` | 后端集成测试 | 创建 |
| `src/router/index.ts` | `/debug` 路由 | 修改 |
| `src/views/HomeView.vue` | 首页入口链接 | 修改 |
| `src/views/DebugView.vue` | 调试界面（文件选择+页面切换+引擎切换+拖动文字框） | 创建 |

---

### 任务 1：后端调试提取模块（TDD）

**文件：**
- 创建：`src-tauri/src/pdf/debug_extract.rs`
- 修改：`src-tauri/src/pdf/mod.rs`（加模块声明）
- 测试：`src-tauri/tests/debug_extract_test.rs`

- [ ] **步骤 1：编写失败的测试**

创建 `src-tauri/tests/debug_extract_test.rs`：

```rust
#![cfg(feature = "pdfplumber")]

use invoice_reimbursement_lib::pdf::debug_extract::{debug_extract_texts, DebugTextItem};
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{DATA_DIR}\\{normalized}")
}

#[test]
fn test_debug_extract_returns_structure_with_scaled_coords() {
    let pdf_path = data_path("发票与行程单\\滴滴电子发票A.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    // ocr_engine = None：OCR 数组应为空，不阻塞 pdfplumber/zpdf
    let result = debug_extract_texts(&pdf_path, 200, None).expect("extract should succeed");

    assert!(!result.pages.is_empty(), "should have at least one page");
    let page = &result.pages[0];
    assert!(!page.image.is_empty(), "image base64 should be present");
    assert!(page.image.starts_with("data:image/png;base64,"), "image should be png data uri");
    assert!(page.width > 0 && page.height > 0, "image dimensions should be set");

    // pdfplumber 应提取到文字（这是文字型 PDF）
    assert!(!page.pdfplumber.is_empty(), "pdfplumber should extract words");

    // 所有 pdfplumber 坐标必须在图片像素范围内（验证缩放正确）
    for item in &page.pdfplumber {
        assert_in_bounds(item, page.width, page.height, "pdfplumber");
    }

    // zpdf 也应提取到文字
    assert!(!page.zpdf.is_empty(), "zpdf should extract words");
    for item in &page.zpdf {
        assert_in_bounds(item, page.width, page.height, "zpdf");
    }

    // OCR 引擎未提供，应为空数组（不报错）
    assert!(page.ocr.is_empty(), "ocr should be empty when engine is None");
}

#[test]
fn test_debug_extract_different_dpi_scales_coords_proportionally() {
    let pdf_path = data_path("发票与行程单\\滴滴电子发票A.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    let r150 = debug_extract_texts(&pdf_path, 150, None).unwrap();
    let r300 = debug_extract_texts(&pdf_path, 300, None).unwrap();

    let p150 = &r150.pages[0];
    let p300 = &r300.pages[0];

    // 图片尺寸应随 DPI 线性缩放
    let w_ratio = p300.width as f64 / p150.width as f64;
    assert!((w_ratio - 2.0).abs() < 0.05, "300/150 width ratio should be ~2.0, got {w_ratio}");

    // pdfplumber 第一个文字框的 x 也应随 DPI 线性缩放
    let x150 = p150.pdfplumber[0].x;
    let x300 = p300.pdfplumber[0].x;
    let x_ratio = x300 / x150;
    assert!((x_ratio - 2.0).abs() < 0.05, "pdfplumber x should scale ~2x, got {x_ratio}");
}

fn assert_in_bounds(item: &DebugTextItem, w: u32, h: u32, label: &str) {
    let (w, h) = (w as f64, h as f64);
    assert!(
        item.x >= -1.0 && item.x <= w,
        "{label} x={} out of [0, {}]", item.x, w
    );
    assert!(
        item.y >= -1.0 && item.y <= h,
        "{label} y={} out of [0, {}]", item.y, h
    );
    assert!(item.w > 0.0 && item.h > 0.0, "{label} w/h should be positive");
    assert!(
        item.x + item.w <= w + 1.0,
        "{label} x+w={} exceeds {}", item.x + item.w, w
    );
    assert!(
        item.y + item.h <= h + 1.0,
        "{label} y+h={} exceeds {}", item.y + item.h, h
    );
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test --features pdfplumber --test debug_extract_test`
预期：编译失败，报错 `could not find module debug_extract` 或 `unresolved import`

- [ ] **步骤 3：在 `pdf/mod.rs` 加模块声明**

读取 `src-tauri/src/pdf/mod.rs`，在现有 `pub mod ...` 列表中加一行：

```rust
pub mod debug_extract;
```

- [ ] **步骤 4：创建 `src-tauri/src/pdf/debug_extract.rs` 实现核心函数**

```rust
use crate::ocr::engine::OcrEngine;
use crate::ocr::OcrTextItem;
use crate::pdf::image_embedder::render_pdf_to_rgb_images;
use crate::pdf::text_extractor::{extract_raw_words_debug, extract_text_with_zpdf};
use serde::{Deserialize, Serialize};

/// 调试界面单个文字项：坐标已统一到渲染图片像素空间（左上角 + 宽高）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugTextItem {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub confidence: f64,
}

/// 调试界面单页结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugPage {
    /// "data:image/png;base64,..."
    pub image: String,
    pub width: u32,
    pub height: u32,
    pub pdfplumber: Vec<DebugTextItem>,
    pub zpdf: Vec<DebugTextItem>,
    pub ocr: Vec<DebugTextItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugTextResult {
    pub pages: Vec<DebugPage>,
}

/// 提取 PDF 文字并统一坐标到渲染图片像素空间。
/// `ocr_engine` 为 None 时 OCR 数组为空（不报错）。
pub fn debug_extract_texts(
    pdf_path: &str,
    dpi: u32,
    ocr_engine: Option<&mut OcrEngine>,
) -> Result<DebugTextResult, String> {
    // 1. 渲染各页为图片 → base64
    let images = render_pdf_to_rgb_images(pdf_path, dpi)?;
    let page_count = images.len();

    // 2. pdfplumber 原始 word（PDF 点坐标），按页分组
    let pdfplumber_by_page = extract_pdfplumber_by_page(pdf_path, page_count)?;

    // 3. zpdf 文字（PDF 点坐标，已 y-down），按页分组
    let zpdf_by_page = extract_zpdf_by_page(pdf_path, page_count)?;

    // 4. OCR（200DPI 像素坐标），按页分组
    let ocr_by_page = if let Some(engine) = ocr_engine {
        extract_ocr_by_page(engine, pdf_path, page_count)?
    } else {
        vec![Vec::new(); page_count]
    };

    // 5. 组装：统一坐标到各页图片像素空间
    let scale_pt = dpi as f64 / 72.0; // PDF点 → 渲染像素
    let scale_ocr = dpi as f64 / 200.0; // OCR像素(200DPI) → 渲染像素

    let mut pages = Vec::with_capacity(page_count);
    for (i, img) in images.iter().enumerate() {
        let (w, h) = (img.width(), img.height());
        let image = rgb_to_png_data_uri(img)?;

        let pdfplumber: Vec<DebugTextItem> = pdfplumber_by_page
            .get(i)
            .map(|words| {
                words
                    .iter()
                    .map(|(text, x0, top, x1, bottom, _pn)| DebugTextItem {
                        text: text.clone(),
                        x: x0 * scale_pt,
                        y: top * scale_pt,
                        w: (x1 - x0) * scale_pt,
                        h: (bottom - top) * scale_pt,
                        confidence: 1.0,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let zpdf: Vec<DebugTextItem> = zpdf_by_page
            .get(i)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|it| parse_box_to_debug(it, scale_pt))
                    .collect()
            })
            .unwrap_or_default();

        let ocr: Vec<DebugTextItem> = ocr_by_page
            .get(i)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|it| parse_box_to_debug(it, scale_ocr))
                    .collect()
            })
            .unwrap_or_default();

        pages.push(DebugPage {
            image,
            width: w,
            height: h,
            pdfplumber,
            zpdf,
            ocr,
        });
    }

    Ok(DebugTextResult { pages })
}

/// pdfplumber 原始 word 按页分组（page_number 从 1 开始）
#[cfg(feature = "pdfplumber")]
fn extract_pdfplumber_by_page(
    pdf_path: &str,
    page_count: usize,
) -> Result<Vec<Vec<(String, f64, f64, f64, f64, u32)>>, String> {
    let words = extract_raw_words_debug(pdf_path).unwrap_or_default();
    let mut by_page: Vec<Vec<_>> = vec![Vec::new(); page_count];
    for w in words {
        let idx = (w.5 as usize).saturating_sub(1);
        if idx < page_count {
            by_page[idx].push(w);
        }
    }
    Ok(by_page)
}

#[cfg(not(feature = "pdfplumber"))]
fn extract_pdfplumber_by_page(
    _pdf_path: &str,
    page_count: usize,
) -> Result<Vec<Vec<(String, f64, f64, f64, f64, u32)>>, String> {
    Ok(vec![Vec::new(); page_count])
}

/// zpdf 文字按页分组。extract_text_with_zpdf 返回扁平 Vec<OcrTextItem>，
/// 无页码字段，按顺序无法分页——这里重新用 zpdf 按页提取。
fn extract_zpdf_by_page(
    pdf_path: &str,
    page_count: usize,
) -> Result<Vec<Vec<OcrTextItem>>, String> {
    // ponytail: 复用 extract_text_with_zpdf 会丢失页码分组，这里内联按页提取
    use zpdf::{ContentInterpreter, ImageCache, PdfDocument};

    let data = std::fs::read(pdf_path).map_err(|e| format!("读取 PDF 失败: {}", e))?;
    let doc = PdfDocument::open(data).map_err(|e| format!("解析 PDF 失败: {:?}", e))?;

    let mut by_page: Vec<Vec<OcrTextItem>> = Vec::with_capacity(page_count);
    for i in 0..page_count {
        let page = doc.page(i as u32).map_err(|e| format!("获取页面 {} 失败: {:?}", i, e))?;
        let mut fonts = doc.load_page_fonts(&page);
        let mut img_cache = ImageCache::new();
        let content = doc
            .page_content_bytes(&page)
            .map_err(|e| format!("获取页面 {} 内容失败: {:?}", i, e))?;

        let mut spans: Vec<zpdf::TextSpan> = Vec::new();
        let page_rect = page.effective_box();
        let _ = ContentInterpreter::new(page_rect)
            .with_page_rotation(page.rotate)
            .with_fonts(&mut fonts)
            .with_document(doc.file(), &page.resources)
            .with_images(&mut img_cache)
            .with_text_sink(&mut spans)
            .interpret(&content);

        let mut items = Vec::new();
        for span in &spans {
            let text = span.text.trim();
            if !text.is_empty() {
                let x0 = span.x;
                let y_top = page_rect.y1 - (span.y + span.size as f64);
                let x1 = span.x + span.advance;
                let y_bottom = page_rect.y1 - span.y;
                items.push(OcrTextItem {
                    text: text.to_string(),
                    confidence: 1.0,
                    box_coords: Some(crate::ocr::engine::bbox_to_json(
                        x0, y_top, x1, y_bottom, 1.0,
                    )),
                });
            }
        }
        by_page.push(items);
    }
    Ok(by_page)
}

/// OCR 按页分组（recognize_pdf 返回 OcrPdfResponse { pages: [{page, texts}] }）
fn extract_ocr_by_page(
    engine: &mut OcrEngine,
    pdf_path: &str,
    page_count: usize,
) -> Result<Vec<Vec<OcrTextItem>>, String> {
    let resp = engine.recognize_pdf(pdf_path);
    match resp {
        Ok(r) => {
            let mut by_page = vec![Vec::new(); page_count];
            for p in r.pages {
                let idx = (p.page as usize).saturating_sub(1);
                if idx < page_count {
                    by_page[idx] = p.texts;
                }
            }
            Ok(by_page)
        }
        Err(_) => Ok(vec![Vec::new(); page_count]),
    }
}

/// 从 OcrTextItem.box_coords（{points:[{x,y}*4], box_score}）解析为 DebugTextItem
fn parse_box_to_debug(item: &OcrTextItem, scale: f64) -> Option<DebugTextItem> {
    let coords = item.box_coords.as_ref()?;
    let points = coords.get("points")?.as_array()?;
    if points.len() < 4 {
        return None;
    }
    let mut xs = [0.0f64; 4];
    let mut ys = [0.0f64; 4];
    for (i, p) in points.iter().take(4).enumerate() {
        xs[i] = p.get("x")?.as_f64()?;
        ys[i] = p.get("y")?.as_f64()?;
    }
    let x0 = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let x1 = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let y0 = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let y1 = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Some(DebugTextItem {
        text: item.text.clone(),
        x: x0 * scale,
        y: y0 * scale,
        w: (x1 - x0) * scale,
        h: (y1 - y0) * scale,
        confidence: item.confidence,
    })
}

/// RgbImage → PNG → base64 data URI
fn rgb_to_png_data_uri(img: &image::RgbImage) -> Result<String, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("PNG 编码失败: {:?}", e))?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    Ok(format!("data:image/png;base64,{b64}"))
}
```

- [ ] **步骤 5：运行测试验证通过**

运行：`cargo test --features pdfplumber --test debug_extract_test`
预期：2 个测试 PASS（若 PDF 不存在则 SKIP，但仍算 PASS）

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/pdf/debug_extract.rs src-tauri/src/pdf/mod.rs src-tauri/tests/debug_extract_test.rs
git commit -m "feat: 新增 PDF 文字提取调试模块（三引擎坐标统一）"
```

---

### 任务 2：Tauri command 包装 + 注册

**文件：**
- 修改：`src-tauri/src/lib.rs`（加 command + 注册）

- [ ] **步骤 1：在 `lib.rs` 加 command 函数**

在 `render_pdf_preview` 函数之后（约 `lib.rs:509` 行后）插入：

```rust
// 调试：提取 PDF 文字并返回三引擎坐标对比数据
#[tauri::command]
async fn debug_extract_texts(
    state: tauri::State<'_, AppState>,
    file_path: String,
    dpi: Option<u32>,
) -> Result<crate::pdf::debug_extract::DebugTextResult, String> {
    let dpi = dpi.unwrap_or(200);
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("文件不存在: {}", file_path));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "pdf" {
        return Err(format!("调试界面仅支持 PDF，收到: .{}", ext));
    }
    // OCR 引擎可能未初始化（模型未下载），传 Some 让后端自行降级
    let mut engine = state.ocr_engine.lock().await;
    let engine_ref: Option<&mut OcrEngine> = if engine.health().unwrap_or(false) {
        Some(&mut *engine)
    } else {
        None
    };
    crate::pdf::debug_extract::debug_extract_texts(&file_path, dpi, engine_ref)
}
```

- [ ] **步骤 2：在 `invoke_handler` 注册 command**

在 `lib.rs:583-609` 的 `tauri::generate_handler![...]` 列表中，`render_pdf_preview,` 之后加一行：

```rust
            debug_extract_texts,
```

- [ ] **步骤 3：编译验证**

运行：`cargo build --features pdfplumber`
预期：编译成功，无错误

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: 注册 debug_extract_texts Tauri command"
```

---

### 任务 3：前端路由 + 首页入口

**文件：**
- 修改：`src/router/index.ts`
- 修改：`src/views/HomeView.vue`

- [ ] **步骤 1：在路由加 `/debug`**

修改 `src/router/index.ts`，在 routes 数组末尾加：

```ts
  { path: '/debug', name: 'debug', component: () => import('../views/DebugView.vue') },
```

完整文件应为：

```ts
import { createRouter, createWebHashHistory } from 'vue-router'

const routes = [
  { path: '/', name: 'home', component: () => import('../views/HomeView.vue') },
  { path: '/import', name: 'import', component: () => import('../views/ImportView.vue') },
  { path: '/match', name: 'match', component: () => import('../views/MatchView.vue') },
  { path: '/export', name: 'export', component: () => import('../views/ExportView.vue') },
  { path: '/debug', name: 'debug', component: () => import('../views/DebugView.vue') },
]

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
})
```

- [ ] **步骤 2：在 HomeView 快速操作加入口**

修改 `src/views/HomeView.vue`，把"快速操作"的 grid 从 `grid-cols-3` 改为 `grid-cols-4`，并在"开始匹配"之后加一个调试入口 `<router-link>`：

将 `HomeView.vue:70` 的 `<div class="grid grid-cols-3 gap-4">` 改为：
```html
      <div class="grid grid-cols-4 gap-4">
```

在 `HomeView.vue:85` 的"开始匹配" `</router-link>` 之后、`</div>` 之前加：
```html
        <router-link to="/debug"
          class="flex flex-col items-center gap-2 p-4 rounded-lg border-2 border-dashed border-gray-200 hover:border-amber-400 hover:bg-amber-50 transition-colors">
          <span class="text-2xl">🔍</span>
          <span class="text-sm font-medium">文字提取调试</span>
        </router-link>
```

- [ ] **步骤 3：Commit**

```bash
git add src/router/index.ts src/views/HomeView.vue
git commit -m "feat: 新增 /debug 路由与首页入口"
```

---

### 任务 4：DebugView.vue 调试界面

**文件：**
- 创建：`src/views/DebugView.vue`

- [ ] **步骤 1：创建 DebugView.vue**

```vue
<template>
  <div class="max-w-6xl mx-auto">
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-2xl font-bold">文字提取调试</h2>
      <router-link to="/" class="text-sm text-blue-500 hover:underline">← 返回首页</router-link>
    </div>

    <!-- 工具栏 -->
    <div class="bg-white rounded-lg border p-4 shadow-sm mb-4 flex flex-wrap items-center gap-4">
      <button @click="pickPdf"
        class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors text-sm">
        选择 PDF
      </button>
      <span v-if="fileName" class="text-sm text-gray-600 truncate max-w-xs">{{ fileName }}</span>

      <div v-if="pages.length" class="flex items-center gap-2">
        <button @click="prevPage" :disabled="currentPage === 0"
          class="px-2 py-1 border rounded text-sm disabled:opacity-40">上一页</button>
        <span class="text-sm">{{ currentPage + 1 }} / {{ pages.length }}</span>
        <button @click="nextPage" :disabled="currentPage >= pages.length - 1"
          class="px-2 py-1 border rounded text-sm disabled:opacity-40">下一页</button>
      </div>

      <div v-if="pages.length" class="flex items-center gap-3 ml-auto">
        <label class="flex items-center gap-1 text-sm">
          <input type="checkbox" v-model="showPdfplumber" class="accent-blue-500">
          <span class="text-blue-600">pdfplumber</span>
        </label>
        <label class="flex items-center gap-1 text-sm">
          <input type="checkbox" v-model="showZpdf" class="accent-green-500">
          <span class="text-green-600">zpdf</span>
        </label>
        <label class="flex items-center gap-1 text-sm">
          <input type="checkbox" v-model="showOcr" class="accent-red-500">
          <span class="text-red-600">OCR</span>
        </label>
      </div>
    </div>

    <!-- 错误提示 -->
    <div v-if="error" class="bg-red-50 border border-red-200 text-red-700 rounded-lg p-3 mb-4 text-sm">
      {{ error }}
    </div>

    <!-- 加载中 -->
    <div v-if="loading" class="text-center py-12 text-gray-400">正在提取文字…</div>

    <!-- 主区域：PDF 底图 + 文字框叠加 -->
    <div v-if="currentPageData" ref="stageRef"
      class="relative inline-block bg-gray-100 rounded-lg border shadow-sm select-none"
      :style="{ width: displayWidth + 'px' }">
      <img :src="currentPageData.image" :width="displayWidth"
        class="block rounded-lg" draggable="false" />

      <!-- 文字框层 -->
      <div class="absolute inset-0">
        <div v-for="(item, idx) in visibleBoxes" :key="idx"
          class="absolute border-2 cursor-move"
          :style="boxStyle(item)"
          @mousedown="startDrag($event, idx)"
          @mouseenter="hoveredIdx = idx"
          @mouseleave="hoveredIdx = -1">
          <span class="absolute left-0 top-0 px-0.5 text-xs leading-tight whitespace-nowrap overflow-hidden"
            :style="{ fontSize: Math.max(8, item.h * scale) + 'px', lineHeight: (item.h * scale) + 'px', color: item.color, maxWidth: (item.w * scale) + 'px' }">
            {{ item.text }}
          </span>
        </div>
      </div>

      <!-- 悬停坐标提示 -->
      <div v-if="hoveredIdx >= 0 && visibleBoxes[hoveredIdx]"
        class="absolute bottom-0 left-0 bg-black/70 text-white text-xs px-2 py-1 rounded-tr pointer-events-none">
        {{ visibleBoxes[hoveredIdx].engine }}: "{{ visibleBoxes[hoveredIdx].text }}" x={{ visibleBoxes[hoveredIdx].origX.toFixed(1) }} y={{ visibleBoxes[hoveredIdx].origY.toFixed(1) }} w={{ visibleBoxes[hoveredIdx].origW.toFixed(1) }} h={{ visibleBoxes[hoveredIdx].origH.toFixed(1) }}
      </div>
    </div>

    <p v-if="currentPageData && !visibleBoxes.length" class="text-sm text-gray-400 mt-4">
      当前页无可显示文字框（勾选上方引擎或检查提取结果）。
    </p>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

interface DebugTextItem {
  text: string
  x: number
  y: number
  w: number
  h: number
  confidence: number
}
interface DebugPage {
  image: string
  width: number
  height: number
  pdfplumber: DebugTextItem[]
  zpdf: DebugTextItem[]
  ocr: DebugTextItem[]
}
interface DebugTextResult {
  pages: DebugPage[]
}

const ENGINE_COLORS = {
  pdfplumber: 'border-blue-500 text-blue-700',
  zpdf: 'border-green-500 text-green-700',
  ocr: 'border-red-500 text-red-700',
}
const ENGINE_COLOR_HEX = {
  pdfplumber: '#2563eb',
  zpdf: '#16a34a',
  ocr: '#dc2626',
}

const fileName = ref('')
const loading = ref(false)
const error = ref('')
const pages = ref<DebugPage[]>([])
const currentPage = ref(0)
const displayWidth = ref(900)

const showPdfplumber = ref(true)
const showZpdf = ref(true)
const showOcr = ref(true)

const hoveredIdx = ref(-1)
const stageRef = ref<HTMLElement | null>(null)

// 拖动状态：每个引擎独立的偏移量（按 engine+index 标识）
// ponytail: 临时拖动不保存，切换页面/引擎时重置
const dragOffsets = ref<Record<string, { dx: number; dy: number }>>({})
const dragState = ref<{ key: string; startMouseX: number; startMouseY: number; startDx: number; startDy: number } | null>(null)

const currentPageData = computed(() => pages.value[currentPage.value] ?? null)

const scale = computed(() => {
  const p = currentPageData.value
  if (!p || p.width === 0) return 1
  return displayWidth.value / p.width
})

interface VisibleBox extends DebugTextItem {
  engine: string
  color: string
  origX: number
  origY: number
  origW: number
  origH: number
  offsetX: number
  offsetY: number
}

const visibleBoxes = computed<VisibleBox[]>(() => {
  const p = currentPageData.value
  if (!p) return []
  const out: VisibleBox[] = []
  const push = (items: DebugTextItem[], engine: string) => {
    for (let i = 0; i < items.length; i++) {
      const it = items[i]
      const key = `${engine}-${currentPage.value}-${i}`
      const off = dragOffsets.value[key] ?? { dx: 0, dy: 0 }
      out.push({
        ...it,
        engine,
        color: ENGINE_COLOR_HEX[engine as keyof typeof ENGINE_COLOR_HEX],
        origX: it.x, origY: it.y, origW: it.w, origH: it.h,
        offsetX: off.dx, offsetY: off.dy,
      })
    }
  }
  if (showPdfplumber.value) push(p.pdfplumber, 'pdfplumber')
  if (showZpdf.value) push(p.zpdf, 'zpdf')
  if (showOcr.value) push(p.ocr, 'ocr')
  return out
})

function boxStyle(item: VisibleBox) {
  const s = scale.value
  return {
    left: (item.x * s + item.offsetX) + 'px',
    top: (item.y * s + item.offsetY) + 'px',
    width: (item.w * s) + 'px',
    height: (item.h * s) + 'px',
    borderColor: item.color,
    backgroundColor: item.color + '15',
  }
}

function startDrag(e: MouseEvent, idx: number) {
  const box = visibleBoxes.value[idx]
  if (!box) return
  const key = `${box.engine}-${currentPage.value}-${visibleBoxes.value
    .filter(b => b.engine === box.engine)
    .indexOf(box)}`
  const off = dragOffsets.value[key] ?? { dx: 0, dy: 0 }
  dragState.value = {
    key,
    startMouseX: e.clientX,
    startMouseY: e.clientY,
    startDx: off.dx,
    startDy: off.dy,
  }
  e.preventDefault()
}

function onMouseMove(e: MouseEvent) {
  const ds = dragState.value
  if (!ds) return
  dragOffsets.value = {
    ...dragOffsets.value,
    [ds.key]: {
      dx: ds.startDx + (e.clientX - ds.startMouseX),
      dy: ds.startDy + (e.clientY - ds.startMouseY),
    },
  }
}

function onMouseUp() {
  dragState.value = null
}

window.addEventListener('mousemove', onMouseMove)
window.addEventListener('mouseup', onMouseUp)
onUnmounted(() => {
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
})

function prevPage() {
  if (currentPage.value > 0) {
    currentPage.value--
    dragOffsets.value = {}
  }
}
function nextPage() {
  if (currentPage.value < pages.value.length - 1) {
    currentPage.value++
    dragOffsets.value = {}
  }
}

async function pickPdf() {
  error.value = ''
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    })
    if (!selected) return
    const filePath = typeof selected === 'string' ? selected : selected.path
    if (!filePath) return
    fileName.value = filePath.split(/[\\/]/).pop() ?? filePath
    loading.value = true
    pages.value = []
    currentPage.value = 0
    dragOffsets.value = {}
    const result = await invoke<DebugTextResult>('debug_extract_texts', {
      filePath,
      dpi: 200,
    })
    pages.value = result.pages
    if (!result.pages.length) error.value = '未提取到任何页面'
  } catch (e) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}
</script>
```

- [ ] **步骤 2：确认 dialog 插件已安装**

运行：`grep '"@tauri-apps/plugin-dialog"' package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json`
预期：能找到 `@tauri-apps/plugin-dialog` 依赖。若前端依赖存在但 `open` 不可用，检查 `src-tauri/tauri.conf.json` 的 `plugins` 配置。

若未安装，运行：
```bash
npm install @tauri-apps/plugin-dialog
```
并在 `src-tauri/Cargo.toml` 加 `tauri-plugin-dialog`，在 `lib.rs` 的 `.plugin(...)` 注册（参考项目现有插件注册方式）。

- [ ] **步骤 3：前端类型检查**

运行：`npm run type-check`（或 `npx vue-tsc --noEmit`）
预期：无类型错误

- [ ] **步骤 4：Commit**

```bash
git add src/views/DebugView.vue
git commit -m "feat: 新增 PDF 文字提取调试对比界面"
```

---

### 任务 5：端到端手动验证

- [ ] **步骤 1：启动开发环境**

运行：`npm run tauri dev`
预期：应用启动，无编译错误

- [ ] **步骤 2：验证首页入口**

在应用中确认首页"快速操作"有"文字提取调试"入口，点击跳转到 `/debug`。

- [ ] **步骤 3：验证 PDF 加载与三引擎叠加**

点击"选择 PDF"，选 `data/发票与行程单/滴滴电子发票A.pdf`：
- 预期：显示 PDF 第一页图片，上面叠加蓝色（pdfplumber）、绿色（zpdf）文字框
- OCR 若模型未加载，红色框不出现（正常）
- 取消勾选 pdfplumber，蓝色框消失；重新勾选恢复

- [ ] **步骤 4：验证拖动**

鼠标按住一个文字框拖动：
- 预期：文字框跟随鼠标移动
- 切换页面再切回：拖动偏移重置

- [ ] **步骤 5：验证悬停坐标**

鼠标悬停文字框：
- 预期：左下角显示 `engine: "text" x=.. y=.. w=.. h=..`

- [ ] **步骤 6：验证页面切换**

多页 PDF 用"上一页/下一页"切换：
- 预期：图片和文字框同步更新

- [ ] **步骤 7：Commit 验证记录（可选）**

无需 commit，手动验证通过即完成。

---

## 自检

**规格覆盖度：**
- ✅ 选择 PDF → 任务 4 `pickPdf`
- ✅ PDF 渲染图片作底图 → 任务 1 `render_pdf_to_rgb_images` + 任务 4 `<img>`
- ✅ 三引擎可切换 → 任务 4 checkbox + `visibleBoxes`
- ✅ 文字框可拖动 → 任务 4 `startDrag`/`onMouseMove`
- ✅ 临时拖动不保存 → 任务 4 `dragOffsets` 切页重置
- ✅ pdfplumber=蓝/zpdf=绿/OCR=红 → 任务 4 `ENGINE_COLOR_HEX`
- ✅ 悬停显示坐标 → 任务 4 悬停提示
- ✅ 后端统一坐标 → 任务 1 `scale_pt`/`scale_ocr`
- ✅ 单引擎失败不阻塞 → 任务 1 各引擎 `unwrap_or_default`/`Err => empty`
- ✅ OCR 未加载为空 → 任务 2 `engine_ref = None` + 任务 1 `Err => empty`
- ✅ 路由 `/debug` → 任务 3
- ✅ 首页入口 → 任务 3
- ✅ 后端测试 → 任务 1

**占位符扫描：** 无 TODO/待定。所有代码步骤含完整代码。

**类型一致性：**
- `DebugTextItem`/`DebugPage`/`DebugTextResult` 在任务 1 定义，任务 2 command 返回 `DebugTextResult`，任务 4 前端 interface 匹配（字段名 text/x/y/w/h/confidence、image/width/height/pdfplumber/zpdf/ocr、pages）✅
- `debug_extract_texts(pdf_path, dpi, ocr_engine)` 签名任务 1 定义，任务 2 调用参数一致 ✅
- 前端 invoke 参数 `filePath`/`dpi` 对应 Rust `file_path`/`dpi`（Tauri 自动 camelCase 转换）✅
