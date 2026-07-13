# PDF 文字提取调试对比界面 设计文档

日期：2026-07-13
状态：已批准

## 目的

调试优化 PDF 文字提取效果。提供一个界面，选择 PDF 后以渲染的页面图片为底图，叠加显示各文字提取引擎返回的文字框和坐标，支持拖动文字框与底图对齐比较，从而直观检查文字提取的准确性和坐标偏差。

## 范围

仅调试/可视化界面，不修改现有发票/行程单解析管线。临时拖动不保存、不导出修正坐标。

## 文字提取来源

三个有坐标的引擎，可勾选切换/叠加：

| 引擎 | 颜色 | 坐标来源 |
|------|------|----------|
| pdfplumber | 蓝色 | Word 级 BBox（PDF 点单位） |
| zpdf | 绿色 | Form XObject 文字（屏幕坐标） |
| OCR (PaddleOCR) | 红色 | 像素坐标 |

parangi 无坐标，不纳入本界面。

## 技术方案

### 前端叠加方式：HTML div 绝对定位

PDF 页面图片作底图，每个文字框是绝对定位的 `<div>`，文字用 DOM 渲染。拖动用原生 mousedown/mousemove。一页几十个 word，性能无压力。文字清晰可选可复制，实现最简单。

### 路由

新增 `/debug` 路由 → `DebugView.vue`。从首页（HomeView）加入口链接。

### 后端

新增 Tauri command：`debug_extract_texts(file_path: String, dpi: Option<u32>) -> DebugTextResult`（`dpi` 缺省时用 200，与 OCR 内部渲染一致）

职责：
1. 渲染 PDF 各页为图片（复用 `image_embedder.rs::render_pdf_to_rgb_images`）→ base64 PNG data URI
2. 调用三个引擎提取文字+坐标：
   - pdfplumber：复用 `text_extractor.rs::extract_raw_words_debug`（返回 text, x0, top, x1, bottom, page）
   - zpdf：复用 `text_extractor.rs::extract_text_with_zpdf`
   - OCR：复用 `ocr_recognize_pdf`（`OcrPdfResponse`）
3. **后端统一坐标到图片像素空间**：
   - pdfplumber：PDF 点坐标 × (image_width / pdf_page_width)
   - zpdf：已是屏幕坐标，按 image_width / render_width 缩放
   - OCR：像素坐标 × (image_width / ocr_image_width)
   - 统一为 `{ x, y, w, h }`（左上角 + 宽高，像素单位，相对渲染图片）

返回结构：

```rust
struct DebugTextResult {
    pages: Vec<DebugPage>,
}

struct DebugPage {
    image: String,        // "data:image/png;base64,..."
    width: u32,           // 图片实际像素宽
    height: u32,          // 图片实际像素高
    pdfplumber: Vec<DebugTextItem>,
    zpdf: Vec<DebugTextItem>,
    ocr: Vec<DebugTextItem>,
}

struct DebugTextItem {
    text: String,
    x: f64,        // 图片像素空间左上角 X
    y: f64,        // 图片像素空间左上角 Y
    w: f64,        // 宽
    h: f64,        // 高
    confidence: f64,  // pdfplumber/zpdf 为 1.0
}
```

### 前端交互

布局：
- 顶部工具栏：文件选择按钮、页面切换（上一页/下一页 + 页码）、引擎显隐 checkbox（pdfplumber=蓝/zpdf=绿/OCR=红）、DPI 选择
- 主区域：PDF 页面图片作底图，上面叠加文字框层

文字框：
- 半透明彩色边框 + 文字标签（文字用 DOM 渲染，font-size 按 h 缩放）
- 可鼠标拖动移动（临时，切换页面/引擎/文件时重置）
- 悬停显示原始坐标值（x, y, w, h）
- 多引擎叠加时，同位置文字框会重叠，拖动可分开查看

坐标对齐：
- 后端统一到图片像素空间后，前端按 `显示宽度 / 图片实际宽度` 缩放比换算 CSS left/top/width/height
- 拖动只改 div 的 left/top，不涉及坐标回写

### 错误处理

- PDF 打开失败：显示错误提示
- 单个引擎提取失败：该引擎返回空数组，不阻塞其他引擎显示
- OCR 引擎未加载模型：OCR 数组为空，界面提示"OCR 模型未加载"

### 测试

- 后端：新增 `tests/debug_extract_test.rs`，用一个测试 PDF 验证 `debug_extract_texts` 返回结构正确、坐标在图片像素范围内
- 前端：手动验证（调试工具，无需单元测试）

## 关键复用点

| 复用 | 文件 | 函数 |
|------|------|------|
| PDF 渲染图片 | `src-tauri/src/pdf/image_embedder.rs` | `render_pdf_to_rgb_images` |
| pdfplumber 原始 word | `src-tauri/src/pdf/text_extractor.rs` | `extract_raw_words_debug` |
| zpdf 文字提取 | `src-tauri/src/pdf/text_extractor.rs` | `extract_text_with_zpdf` |
| OCR | `src-tauri/src/ocr/engine.rs` | `OcrEngine::recognize_pdf` |
| base64 编码 | `src-tauri/src/lib.rs` | `render_pdf_preview` 中的编码逻辑 |

## 不做的事（YAGNI）

- 不保存/导出修正坐标
- 不纳入 parangi（无坐标）
- 不做文字框编辑/删除
- 不做批量 PDF 对比
- 不做单元测试前端组件
