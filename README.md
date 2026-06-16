# 发票报销自动化助手

> 个人出差报销自动化桌面工具 — 发票 OCR 识别、支付账单匹配、报销表单一键生成

---

## 功能特性

- **发票 OCR 识别**：支持拍照、PDF、链接导入，自动提取金额/销售方/日期/发票号
- **行程单解析**：OCR 坐标 + 纯文本交叉验证，支持天府通/滴滴/高德多格式
- **智能分类**：自动识别高铁、飞机、住宿、市内交通、餐饮等类别
- **支付账单导入**：支持微信/支付宝导出的 CSV、XLSX 格式
- **智能匹配引擎**：一对一 / 一对多（打车行程）自动匹配，支持误差容忍
- **报销表单生成**：一键生成 PDF 报销表单 + 发票-支付对照 PDF
- **完全离线**：OCR 使用本地 PaddleOCR 模型，数据不上传云端

---

## 快速开始

### 环境要求

- Node.js 18+
- Rust 1.75+
- 系统依赖（Linux）：
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
  ```

### 安装与运行

```bash
# 克隆项目
git clone git@github.com:chenfuxu920/invoice-reimbursement.git
cd invoice-reimbursement

# 安装前端依赖
npm install

# 开发模式运行
npm run tauri dev

# 生产构建
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。

---

## 项目结构

```
invoice-reimbursement/
├── src/                          # 前端 Vue 3
│   ├── components/               # UI 组件
│   │   ├── InvoiceDropZone.vue   # 发票拖拽上传
│   │   ├── InvoiceCard.vue       # 发票卡片
│   │   ├── InvoiceDetailModal.vue # 发票详情弹窗
│   │   ├── BillImporter.vue      # 账单导入
│   │   ├── PaymentTable.vue      # 支付记录表格
│   │   ├── PaymentDetailModal.vue # 支付详情弹窗
│   │   ├── MatchCard.vue         # 匹配结果卡片
│   │   ├── MatchAdjustDialog.vue # 手动调整对话框
│   │   ├── ReimbursementForm.vue # 报销表单预览
│   │   ├── ExportButton.vue      # 导出按钮
│   │   └── LoadingOverlay.vue    # 加载遮罩
│   ├── views/                    # 页面视图
│   │   ├── HomeView.vue          # 首页
│   │   ├── ImportView.vue        # 导入页面
│   │   ├── MatchView.vue         # 匹配页面
│   │   └── ExportView.vue        # 导出页面
│   ├── stores/                   # Pinia 状态管理
│   │   ├── index.ts              # Store 入口
│   │   ├── invoice.ts            # 发票状态
│   │   ├── payment.ts            # 支付状态
│   │   └── match.ts              # 匹配状态
│   └── types/                    # TypeScript 类型定义
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── ocr/                  # OCR 引擎
│   │   │   ├── engine.rs         # PaddleOCR v5 封装
│   │   │   ├── structured_output.rs # 结构化输出模型
│   │   │   └── mod.rs
│   │   ├── parser/               # 文档解析器
│   │   │   ├── invoice_parser.rs # 发票文本解析
│   │   │   ├── itinerary_parser.rs # 行程单解析
│   │   │   ├── field_extractors.rs # 多策略字段提取
│   │   │   ├── invoice_type_detector.rs # 发票类型检测
│   │   │   ├── template_manager.rs # 模板管理器
│   │   │   ├── alipay_parser.rs  # 支付宝账单解析
│   │   │   ├── wechat_parser.rs  # 微信账单解析
│   │   │   ├── dedup.rs          # 发票去重
│   │   │   └── link_parser.rs    # 发票链接解析
│   │   ├── matching/             # 匹配引擎
│   │   │   ├── engine.rs         # 核心匹配算法
│   │   │   ├── batch.rs          # 批量匹配
│   │   │   ├── batch_optimizer.rs # 批量优化器
│   │   │   ├── scoring.rs        # 多维度评分
│   │   │   ├── strategy_selector.rs # 策略选择
│   │   │   ├── benchmarks.rs     # 性能基准测试
│   │   │   └── manual.rs         # 手动匹配/调整
│   │   ├── models/               # 数据模型
│   │   │   ├── invoice.rs        # 发票模型
│   │   │   ├── payment.rs        # 支付记录模型
│   │   │   ├── match_result.rs   # 匹配结果模型
│   │   │   ├── reimbursement.rs  # 报销表单模型
│   │   │   └── hotel_standard.rs # 住宿标准查询
│   │   ├── pdf/                  # 报表生成
│   │   │   ├── invoice_pipeline.rs # 发票/行程单解析入口
│   │   │   ├── text_extractor.rs # PDF 文本提取
│   │   │   ├── form_builder.rs   # 报销表单构建
│   │   │   ├── form_generator.rs # 表单 PDF 生成
│   │   │   ├── form_html_generator.rs # 表单 HTML 生成
│   │   │   ├── form_xlsx_generator.rs # 表单 XLSX 生成
│   │   │   ├── comparison_generator.rs # 对照表生成
│   │   │   ├── comparison_html_generator.rs # 对照 HTML 生成
│   │   │   ├── comparison_xlsx_generator.rs # 对照 XLSX 生成
│   │   │   ├── comparison_image_pdf_generator.rs # 含图对照 PDF
│   │   │   └── image_embedder.rs # PDF 页面图片渲染
│   │   └── lib.rs                # Tauri 命令注册
│   ├── models/                   # OCR 模型文件
│   │   ├── ch_PP-OCRv5_mobile_det.onnx
│   │   ├── ch_PP-OCRv5_rec_mobile_infer.onnx
│   │   ├── ch_PP-OCRv3_det_infer.onnx
│   │   ├── ch_PP-OCRv3_rec_infer.onnx
│   │   ├── ch_ppocr_mobile_v2.0_cls_infer.onnx
│   │   ├── PP-OCRv5_mobile_det.mnn
│   │   ├── PP-OCRv5_mobile_rec.mnn
│   │   ├── ppocr_keys_v5.txt
│   │   └── ppocr_keys_v1.txt
│   ├── tests/                    # Rust 集成测试
│   └── bin/                      # 命令行调试工具
├── data/                         # 测试数据（发票/账单样本）
├── config/                       # 发票模板配置
└── docs/                         # 项目文档
```

---

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| **前端框架** | Vue 3 + TypeScript | 响应式 UI |
| **构建工具** | Vite | 快速开发/构建 |
| **样式** | TailwindCSS v4 | 原子化 CSS |
| **状态管理** | Pinia | Vue 3 官方状态库 |
| **桌面框架** | Tauri 2.x | Rust 后端 + WebView 前端 |
| **OCR 引擎** | PaddleOCR v5 (paddle-ocr-rs) | 本地 ONNX 推理 |
| **PDF 生成** | printpdf | Rust PDF 库 |
| **数据解析** | calamine / csv | Excel / CSV 读取 |

---

## 用户工作流

```
1. 导入发票 ──→ 2. 导入账单 ──→ 3. 智能匹配 ──→ 4. 人工确认 ──→ 5. 生成报销
   │                │                │                │                │
   ▼                ▼                ▼                ▼                ▼
 拖拽照片/PDF    微信/支付宝      自动金额匹配     查看/调整配对    PDF 表单
 OCR 自动识别    CSV/XLSX 导入    支持误差容忍     手动修正错误    + 对照清单
```

---

## 测试

```bash
# Rust 单元测试
cd src-tauri && cargo test

# OCR 集成测试（需要模型文件）
cd src-tauri && cargo test -- --ignored

# 前端测试
npm run test

# E2E 测试（需要测试数据）
cd src-tauri && cargo test --test e2e_real_data_test -- --ignored
```

---

## 文档

- [需求规格](docs/REQUIREMENTS.md) — 完整的功能/非功能需求
- [构建说明](docs/BUILD.md) — 各平台构建指南
- [实施计划](docs/plans/2025-05-06-implementation-plan.md) — 详细开发任务分解
- [项目规格](docs/plans/2025-05-06-project-spec.md) — 原始设计文档

---

## License

Private — 个人使用
