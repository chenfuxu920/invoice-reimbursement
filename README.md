# 发票报销助手 InvoiceAssistant

> 完全离线的发票报销自动化桌面工具：文字型 PDF 直接本地解析（无需 OCR），图片/扫描件自动回退 OCR；支持支付账单智能匹配、按行程分趟归类、一键生成报销表单。

[![Build Windows](https://github.com/chenfuxu920/invoice-reimbursement/actions/workflows/build-windows.yml/badge.svg)](https://github.com/chenfuxu920/invoice-reimbursement/actions/workflows/build-windows.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

发票报销助手（InvoiceAssistant）是一款基于 Tauri 2 + Vue 3 的跨平台桌面应用，面向个人出差报销场景。它将「手工对账 + 手填表单」的流程自动化：导入发票与支付账单后，自动完成识别、匹配、分趟归类，最终生成可直接提交的报销表单与发票—支付对照材料。

**完全离线运行**：所有解析、识别与匹配均在本机完成，无需联网即可处理文字型 PDF 发票与各类票据，数据全程不出本机。

---

## 功能特性

- **发票直接解析**：文字型 PDF 本地直接解析（首选路径，无需 OCR），照片与扫描件自动回退本地 OCR，自动提取金额、销售方、开票日期、发票号码等字段
- **行程单解析**：PDF 纯文本提取为主，OCR 坐标解析作交叉验证与补充，兼容天府通、滴滴、高德、火车票等多样式行程单
- **智能分类**：自动识别高铁、飞机、住宿、市内交通、餐饮等费用类别
- **支付账单导入**：支持微信 / 支付宝导出的 XLSX、CSV 账单
- **智能匹配引擎**：一对一 / 一对多（打车行程）自动匹配，内置多维度评分与金额误差容忍，支持批量优化与人工修正
- **按行程分趟**：依据城市与日期将票据自动归入各次出差行程，支持待调整票据人工归趟
- **报销表单生成**：一键导出报销表单 PDF / XLSX / HTML，以及发票—支付对照清单（含票据影像版本）
- **完全离线**：文字型 PDF 本地直接解析、OCR 模型随应用内置，全部处理在本机完成，数据不出本机；内置 GitHub Releases 自动更新（可选联网）

## 工作原理

```
发票（文字型 PDF 直接解析 / 图片·扫描件走 OCR）──▶ 本地解析 ──▶ 结构化发票
                                              │
支付账单（微信/支付宝）──▶ 账单解析 ──▶ 支付记录 ──▶ 智能匹配 ──▶ 分趟归类 ──▶ 报销表单 + 对照清单
                                              ▲
                                   评分引擎 + 金额误差容忍 + 人工调整
```

后端核心链路位于 `src-tauri/src/`：

- **ocr/** — PaddleOCR v5（ONNX 推理）封装，输出结构化识别结果
- **parser/** — 发票 / 行程单 / 微信 / 支付宝账单解析，含去重与发票类型检测
- **pdf/** — PDF 文字提取（pdfplumber）、解析流水线、报销表单与对照清单生成
- **matching/** — 匹配引擎：策略选择、多维度评分、批量优化、人工匹配

行程单与发票的字段提取以「PDF 纯文本提取」为主通道、本地 OCR 坐标为扫描件回退与交叉验证通道，任一通道缺漏的字段由另一通道补全或修正，显著提升金额、时间、销售方等关键字段的准确率。

## 快速开始

### 环境要求

| 依赖 | 版本要求 |
|------|----------|
| Node.js | 18+ |
| Rust | stable（≥ 1.77） |
| Windows | WebView2（Win10/11 自带）、MSVC 构建工具 |

Linux 额外系统依赖：

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

macOS 需要 Xcode Command Line Tools。

### 开发运行

```bash
git clone https://github.com/chenfuxu920/invoice-reimbursement.git
cd invoice-reimbursement

npm install
npm run tauri dev
```

### 生产构建

```bash
# 便携版（免安装，release 配置）
npm run tauri:build

# 便携版（快速配置，关闭 LTO 加快编译）
npm run tauri:build:fast

# NSIS 安装包（含自动更新签名与安装程序）
npm run tauri:build:installer
```

构建产物位于 `src-tauri/target/release/bundle/`。

## 使用流程

```
1. 导入发票 ──▶ 2. 导入账单 ──▶ 3. 智能匹配 ──▶ 4. 分趟归类 ──▶ 5. 生成报销
   │                │                │                │                │
   ▼                ▼                ▼                ▼                ▼
 拖拽 PDF/图片   微信/支付宝      自动金额匹配     按城市/日期      PDF 表单
 直接解析/OCR    XLSX/CSV 导入    误差容忍 + 修正   人工归趟        + 对照清单
```

应用内置六个视图：首页、导入、匹配、导出、调试（PDF 文字提取与三引擎坐标对比）与设置。

## 项目结构

```
invoice-reimbursement/
├── src/                        # 前端（Vue 3 + TypeScript）
│   ├── components/             # UI 组件（拖拽上传、票据卡片、匹配卡片等）
│   ├── views/                  # 首页 / 导入 / 匹配 / 导出 / 调试 / 设置
│   ├── stores/                 # Pinia 状态管理（发票 / 账单 / 匹配）
│   └── types/                  # TypeScript 类型定义
├── src-tauri/                  # Rust 后端
│   ├── src/
│   │   ├── ocr/                # PaddleOCR v5 封装与模型下载
│   │   ├── parser/             # 发票 / 行程单 / 账单解析、去重
│   │   ├── pdf/                # 文本提取、解析流水线、表单与对照清单生成
│   │   ├── matching/           # 匹配引擎、评分、批量优化、分趟
│   │   ├── models/             # 发票 / 支付 / 匹配结果 / 报销数据模型
│   │   └── commands/           # Tauri 命令注册
│   ├── models/                 # PaddleOCR v5 ONNX 模型（随应用分发）
│   ├── tests/                  # Rust 集成测试（含真实票据回归测试）
│   └── bin/                    # 命令行调试工具
├── config/                     # 发票模板配置
├── data/                       # 测试数据（发票 / 账单样本）
├── docs/                       # 项目文档
└── scripts/                    # 构建辅助脚本（版本同步、便携打包）
```

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| 前端框架 | Vue 3 + TypeScript | 组合式 API |
| 构建工具 | Vite 5 | 开发与构建 |
| 样式 | TailwindCSS v4 | 原子化 CSS |
| 状态管理 | Pinia | 官方状态库 |
| 桌面框架 | Tauri 2.x | Rust 后端 + 系统 WebView |
| OCR 引擎 | PaddleOCR v5（ONNX） | 本地推理，模型内置（仅图片/扫描件回退使用） |
| PDF 处理 | pdfplumber / zpdf / lopdf | 文本提取、解析与生成 |
| 电子表格 | calamine / rust_xlsxwriter | XLSX / CSV 读写 |
| 自动更新 | tauri-plugin-updater + minisign | GitHub Releases 分发 |

## 测试

```bash
# Rust 单元与集成测试
cd src-tauri && cargo test

# OCR / E2E 集成测试（需要模型与真实测试数据）
cd src-tauri && cargo test -- --ignored

# 前端测试
npm run test
```

`src-tauri/tests/` 内置多组针对真实票据的回归测试，覆盖增值税发票字段提取保真度、行程单单元格解析、CJK 字体宽度、PDF 内容流兼容性等场景。

## 依赖说明

项目对两个上游 PDF 库维护自建 fork，解决中文票据 PDF 的兼容性问题：

- **pdfplumber-rs**（分支 `cjk-safe-lenient`）：修复 CJK CID 字体乱码、ASCII 半角宽度、`/Contents` 间接引用解析、表格单元格提取等问题，详见 [fork 仓库](https://github.com/chenfuxu920/pdfplumber-rs/tree/cjk-safe-lenient)
- **zpdf**（分支 `cjk-ascii-width`）：修复 Type0 CID 字体 ASCII 字符宽度与 Identity-H 子集字形宽度

## 文档

- [需求规格](docs/REQUIREMENTS.md) — 功能与非功能需求
- [构建说明](docs/BUILD.md) — 各平台构建与打包指南
- [产品说明](PRODUCT.md) — 产品定位与设计原则
- [实施计划](docs/plans/) — 各阶段设计规格与实现计划

## License

[MIT](LICENSE)

Copyright © 2026 白开水
