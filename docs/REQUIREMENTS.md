# 发票报销自动化系统 — 需求规格文档

> 版本：1.1.0 | 更新日期：2026-08-04

---

## 1. 项目概述

### 1.1 背景

个人出差报销流程繁琐：需要收集纸质/电子发票、整理支付记录、手动填写报销表单、粘贴发票-支付对照材料。本项目旨在将此流程自动化。

### 1.2 目标

构建一个**离线桌面应用**，实现：
1. 发票自动识别（文字 PDF 直接解析 / 图片 OCR）与结构化
2. 支付账单自动导入（微信 / 支付宝）
3. 发票与支付记录智能匹配
4. 按出差行程分趟归类
5. 报销表单与发票-支付对照材料一键生成

### 1.3 目标用户

- 出差人员（个人使用）
- 需要按单位报销模板整理发票和支付凭证的场景

### 1.4 设计原则

- **离线优先**：所有处理在本地完成，无需联网（仅模型下载/自动更新可选联网）
- **隐私安全**：数据不上传任何云端服务
- **操作简单**：拖拽导入 → 自动处理 → 一键导出

---

## 2. 功能需求

### 2.1 发票收集与识别

#### 2.1.1 输入来源

| 来源 | 格式 | 说明 |
|------|------|------|
| 电子发票 / 扫描件 | PDF | 文字型 PDF 走 pdfplumber 解析，扫描件/文字不足走 OCR |
| 手机拍照 / 图片 | JPG/PNG/BMP/TIFF | 本地 OCR 识别 |
| 外部链接 | URL | 手动选择“外部链接”来源录入（二维码解析为待实现） |
| 纸质票据 | 手动录入空发票 | 无源文件，手动填写金额/类别等 |

#### 2.1.2 提取字段

| 字段 | 说明 | 必须 |
|------|------|------|
| `amount` | 发票金额（价税合计） | ✅ |
| `seller_name` | 销售方名称 | ✅ |
| `item_name` | 项目名称/商品明细 | ✅ |
| `date` | 开票日期 | ✅ |
| `invoice_number` | 发票号码（用于去重） | ✅ |
| `travel_date` | 票面实际出行日期（火车/机票） | 按类别 |
| `departure_city` / `arrival_city` | 出发/到达城市（火车/机票） | 按类别 |
| `hotel_detail` | 入住/离店日期、晚数、每晚均价（住宿） | 按类别 |
| `toll_travel_time` | 通行时间（高速通行费） | 按类别 |
| `remarks` | 备注栏 | 按类别 |
| `itineraries` | 行程明细（打车行程单） | 按类别 |

#### 2.1.3 智能分类

系统自动分类，类别包括：

| 类别 | 枚举值 | 识别关键词/依据 |
|------|--------|------------|
| 高铁/车船票 | `Train` | 铁路、高铁、火车、客运站、车次 |
| 飞机票 | `Flight` | 航空、机票、航班、旅客运输 |
| 保险费 | `Insurance` | 保险、意外险、航意/航延 |
| 退改签费 | `TicketChange` | 退票、改签 |
| 市内交通 | `CityTransport` | 滴滴、高德、网约车、T3、曹操、公交、地铁 |
| 住宿费 | `Hotel` | 酒店、宾馆、住宿、民宿 |
| 餐饮费 | `Meal` | 餐饮、饭店、食品、餐厅 |
| 高速通行费 | `Toll` | 通行费、过路费、ETC、高速 |
| 其他 | `Other` | 以上均不匹配 |

#### 2.1.4 支持的发票/票据类型

基于 `data/` 目录下的真实测试数据，当前覆盖：

| 类型 | 示例 | 说明 |
|------|------|------|
| 增值税电子发票 / 全电发票 | `dzfp_*.pdf`、`全电发票*.pdf` | 标准字段，单元格提取 |
| 滴滴/高德打车发票 | `滴滴电子发票*.pdf` | 网约车发票 |
| 滴滴/高德行程报销单 | `滴滴出行行程报销单*.pdf` | 含多行程明细 |
| 天府通电子行程单 | `天府通电子行程单.pdf` | 公交/地铁行程 |
| 机票报销凭证 | `【飞猪】*.pdf` | 机票 + 退票手续费 |
| 火车票 / 铁路电子客票 | `火车票/*.pdf` | 含出发/到达站、出行日期 |
| 酒店结账单 / 住宿发票 | `成都九眼桥美居酒店*.pdf`、`住宿/*.pdf` | 住宿明细、晚数 |
| 保险发票 | `保险/*.pdf` | 航空意外险等 |
| 高速通行费发票 | `市内交通/*.pdf` | 通行时间提取 |

#### 2.1.5 OCR 技术要求

- **引擎**：PaddleOCR v5（`ocr-rs` 2.2.2，MNN 推理）
- **模型文件**（三个，随应用内置或从 GitHub Releases 下载）：
  - `PP-OCRv5_mobile_det.mnn`
  - `PP-OCRv5_mobile_rec.mnn`
  - `ppocr_keys_v5.txt`
- **PDF 处理**：文字型 PDF 优先 pdfplumber 提取；扫描件/图片用 zpdf 渲染为图片后逐页 OCR（失败时回退 `pdftoppm`）
- **单张识别时间**：目标 < 3 秒

### 2.2 支付账单导入

#### 2.2.1 支持格式

| 平台 | 格式 | 文件特征 |
|------|------|----------|
| 微信支付 | XLSX | `微信支付账单流水文件*.xlsx` |
| 支付宝 | CSV（GBK） | `支付宝交易明细*.csv` |
| 自动识别 | 二者均可 | 按文件内容嗅探后选择解析器 |

#### 2.2.2 提取字段

| 字段 | 说明 | 必须 |
|------|------|------|
| `transaction_id` | 交易单号 | ✅ |
| `transaction_time` | 交易时间 | ✅ |
| `amount` | 实际支付金额（已扣退款） | ✅ |
| `original_amount` | 原始金额 | ✅ |
| `refund_amount` | 退款金额 | ✅ |
| `discount` | 优惠/补贴金额 | ✅ |
| `merchant_name` | 商户名称 | ✅ |
| `source` | 来源（微信/支付宝） | ✅ |
| `category` | 交易类型 | 可选 |
| `payment_method` | 支付方式 | 可选 |

### 2.3 发票-支付匹配引擎

#### 2.3.1 匹配模式

| 模式 | 场景 | 逻辑 |
|------|------|------|
| **一对一 (OneToOne)** | 高铁票、机票、住宿、高速费 | 1张发票 ↔ 1笔支付，金额+参考日期 |
| **一对多 (OneToMany)** | 打车（行程单+发票） | 1张发票 ↔ N笔支付，行程金额汇总匹配 |
| **手动确认 (ManualConfirmed)** | 自动匹配失败 | 用户手动选择配对 |

#### 2.3.2 匹配算法要点

- `batch_match` 是实际使用的批量匹配入口，按金额降序处理非高速费发票；
- 高速费发票最先单独匹配，失败后可与最近的市内交通行程组合金额匹配，并生成共享支付标记；
- 保险费发票使用同批次机票的支付时间/出行时间窗口进行 4 级匹配；
- 一对一匹配在金额容差内优先选择与参考日期（出行日期/入住日期/通行时间/开票日期）最近的支付；
- 一对多子集搜索会寻找金额差最小的最优子集，而非第一个可行子集；
- 默认容差 ±1.00 元（可配置）。

#### 2.3.3 去重

- 按 `invoice_number` 去重（主要）
- 无发票号时按 `金额 + 日期 + 销售方` 复合键去重

### 2.4 行程单处理（市内交通）

#### 2.4.1 输入

- 打车发票 PDF（含总金额）
- 打车行程单 PDF（含各行程明细）

#### 2.4.2 处理流程

```
行程单 PDF
  │
  ├─ pdfplumber 表格/坐标解析（优先）
  │     ├─ find_tables 单元格解析
  │     ├─ 按页 word 级坐标解析（多页行程单）
  │     └─ 纯文本解析回退
  ├─ OCR 坐标解析（pdfplumber 不可用/文字不足时）
  │
  ▼
 提取 date_time / provider / pickup / dropoff / amount / city
  │
  ▼
 三重交叉验证 + 印制合计金额校验
  │
  ▼
 汇总金额 / 打印合计金额
  │
  ▼
 与发票总额配对（有印制合计时精确匹配，否则容差匹配）
```

### 2.5 报销表单生成

#### 2.5.1 报销类别

```
城市间交通费
├── 车、船票（高铁）
├── 飞机票
├── 保险费
└── 订（退、改签）票

市内交通费（含高速通行费，按日标准封顶）

住宿费（按目的地每晚上限封顶）

伙食补助（按日标准）
```

#### 2.5.2 表单内容

| 字段 | 说明 |
|------|------|
| 基本信息 | 姓名、部职别、到达地点、出差日期、同行人数 |
| 各类别汇总 | 单据张数、申报金额 |
| 支付单号 | 关联的支付交易号 |
| 总计金额 | 所有类别合计 + 伙食补助 |

#### 2.5.3 输出文件

- **报销表单**：PDF / HTML / XLSX
- **发票-支付对照表**：PDF（含票据影像）/ HTML / XLSX
- **按趟导出**：支持多趟出差分文件导出

### 2.6 前端界面

#### 2.6.1 页面结构

| 页面 | 路由 | 功能 |
|------|------|------|
| 首页 | `/` | 项目概览、快捷入口、OCR 状态 |
| 导入 | `/import` | 发票上传 + 账单导入 |
| 匹配 | `/match` | 匹配结果展示 + 手动调整 |
| 导出 | `/export` | 报销表单预览 + 导出 |
| 调试 | `/debug` | PDF 文字提取与 pdfplumber/OCR 坐标对比 |
| 设置 | `/settings` | 报销标准（住宿/市内交通/伙食） |

#### 2.6.2 核心交互

- **拖拽上传**：支持文件拖拽到指定区域
- **实时预览**：发票解析结果实时显示
- **手动调整**：匹配结果可手动修正
- **一键导出**：生成 PDF/XLSX/HTML 并保存到指定位置
- **多趟分趟**：按城市与日期自动分趟，可人工调整

---

## 3. 非功能需求

### 3.1 性能

| 指标 | 要求 |
|------|------|
| 单张发票 OCR | < 3 秒 |
| 单张 PDF（多页） | < 10 秒 |
| 匹配 100 对发票-支付 | < 1 秒 |
| 生成报销 PDF | < 5 秒 |

### 3.2 准确率

| 指标 | 要求 |
|------|------|
| 发票金额识别 | > 95% |
| 发票号识别 | > 90% |
| 自动匹配准确率 | > 90%（在误差容忍范围内） |

### 3.3 离线与隐私

- OCR 模型本地运行，不调用外部 API
- 所有数据本地存储，不上传云端
- 支持完全断网使用（除首次下载模型/检查更新外）

### 3.4 兼容性

| 平台 | 最低版本 |
|------|----------|
| Windows | 10+ |
| macOS | 12+ |
| Ubuntu | 22.04+ |

### 3.5 打包分发

| 平台 | 格式 |
|------|------|
| Windows | 便携版 exe + NSIS 安装程序 |
| macOS | `.dmg` / `.app` |
| Linux | `.deb` / `.AppImage` |

---

## 4. 数据模型

### 4.1 Invoice（发票）

```rust
struct Invoice {
    id: String,
    invoice_number: String,
    amount: f64,
    seller_name: String,
    item_name: String,
    date: NaiveDate,
    travel_date: Option<NaiveDate>,       // 火车/机票出行日期
    category: InvoiceCategory,             // Train/Flight/Insurance/TicketChange/CityTransport/Hotel/Meal/Toll/Other
    source: InvoiceSource,                 // Photo/Pdf/Link/Manual
    itineraries: Vec<Itinerary>,           // 打车行程
    itinerary_file: Option<String>,        // 关联行程单文件
    remarks: String,
    hotel_detail: Option<HotelDetail>,     // 住宿详情
    departure_city: Option<String>,
    arrival_city: Option<String>,
    toll_travel_time: Option<NaiveDateTime>,
}

struct Itinerary {
    date_time: String,
    provider: String,
    pickup: String,
    dropoff: String,
    amount: f64,
    city: String,
    incomplete_fields: Vec<String>,
}

struct HotelDetail {
    check_in: Option<NaiveDate>,
    check_out: Option<NaiveDate>,
    nights: usize,
    nightly_rate: f64,
}
```

### 4.2 PaymentRecord（支付记录）

```rust
struct PaymentRecord {
    id: String,
    transaction_id: String,
    transaction_time: String,
    amount: f64,              // 实际支付金额（已扣退款）
    original_amount: f64,     // 原始金额
    refund_amount: f64,       // 退款金额
    discount: f64,            // 优惠金额
    merchant_name: String,
    source: PaymentSource,    // Wechat / Alipay
    category: String,
    payment_method: String,
}
```

### 4.3 MatchResult（匹配结果）

```rust
struct MatchResult {
    invoice_id: String,
    invoice: Invoice,
    payment_ids: Vec<String>,
    payments: Vec<PaymentRecord>,
    match_type: MatchType,            // OneToOne / OneToMany / Unmatched / ManualConfirmed
    confidence: f64,
    amount_diff: f64,
    itinerary_payment_pairs: Vec<ItineraryPaymentPair>, // 行程→支付显式配对
    shared_payment_ids: Vec<String>,  // 高速费共享支付标记
    shared_from_invoice_id: Option<String>,
}
```

### 4.4 ReimbursementForm（报销表单）

```rust
struct ReimbursementForm {
    name: String,
    department: String,
    destination: String,
    travel_start: String,
    travel_end: String,
    travel_days: usize,
    companions: usize,
    transport_details: Vec<TransportDetail>, // 车船票/飞机票/保险费/退改签
    transport_subtotal: f64,
    city_transport_count: usize,
    city_transport_amount: f64,        // 封顶后
    city_transport_actual_amount: f64,
    hotel_levels: Vec<HotelLevelDetail>,
    hotel_subtotal: f64,
    meal_subsidy: MealSubsidyDetail,
    baggage_amount: f64,
    meal_reimbursement: f64,
    summaries: Vec<CategorySummary>,
    total_amount: f64,
}
```

---

## 5. 技术架构

### 5.1 整体架构

```
┌─────────────────────────────────────────────────┐
│                 前端 (Vue 3 + TS)                │
│     Vite + TailwindCSS 4 + Pinia + vue-router   │
└───────────────────────┬─────────────────────────┘
                        │ Tauri IPC
┌───────────────────────┴─────────────────────────┐
│                 后端 (Rust / Tauri 2)            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │  OCR     │ │  Parser  │ │ Matching │        │
│  │  Engine  │ │  Module  │ │  Engine  │        │
│  └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │  PDF     │ │  Models  │ │  Dedup   │        │
│  │Generator │ │  (Data)  │ │  Module  │        │
│  └──────────┘ └──────────┘ └──────────┘        │
└─────────────────────────────────────────────────┘
```

### 5.2 依赖清单

**Rust (Cargo.toml 实际依赖)**

| 依赖 | 用途 |
|------|------|
| `tauri` 2.x + plugins | 桌面框架、对话框/文件/更新/进程 |
| `ocr-rs` 2.2.2 | PaddleOCR v5 MNN 推理 |
| `pdfplumber`（fork `main`） | PDF 文字/表格/坐标提取 |
| `zpdf` 0.11（fork patch） | PDF 渲染为图片 |
| `medpdf` / `lopdf` / `genpdf` | PDF 生成/解析 |
| `allsorts` / `ttf-parser` | CJK 字体子集化与宽度 |
| `calamine` / `rust_xlsxwriter` | XLSX 读取/写入 |
| `encoding_rs` | 支付宝 CSV GBK 解码 |
| `serde` / `serde_json` | 序列化 |
| `chrono` | 日期处理 |
| `regex` | 正则匹配 |
| `uuid` | ID 生成 |
| `reqwest` / `tokio` / `minisign` | 模型下载与自动更新 |

**前端 (package.json)**

| 依赖 | 用途 |
|------|------|
| `vue` 3.x | UI 框架 |
| `pinia` | 状态管理 |
| `vue-router` | 路由 |
| `tailwindcss` 4.x | 样式 |
| `@tauri-apps/api` + plugins | Tauri API |
| `lucide-vue-next` | 图标 |
| `@tanstack/vue-virtual` | 虚拟列表 |
| `@vueuse/core` | 组合式工具 |

---

## 6. 测试策略

### 6.1 测试层级

| 层级 | 工具 | 覆盖范围 |
|------|------|----------|
| 单元测试 | `cargo test` | 数据模型、解析器、匹配算法 |
| 集成测试 | `cargo test -- --ignored` | OCR 引擎 + 模型文件 + 真实票据 |
| 前端测试 | `npm run test`（vitest） | Store、类型、工具函数 |

### 6.2 测试数据

`data/` 目录包含多类真实票据：增值税/全电发票、滴滴/高德发票与行程单、天府通、机票、火车票、酒店、保险、高速通行费等，以及微信/支付宝账单样本。

### 6.3 测试结果

以实际执行为准；提交/发布前应运行：

```bash
cd src-tauri && cargo test
npm run test
```

---

## 7. 约束与限制

### 7.1 已知限制

- OCR 对手写体、模糊图片识别率较低
- 部分 OCR 乱码时间（如“成都A428”）无法从 OCR 本身恢复，依赖纯文本交叉验证
- 滴滴 page2 表头合并块可能导致 provider/time 边界过宽
- 发票号码/开票日期不在表格内，只能全文正则提取，不要依赖单元格提取

### 7.2 后续扩展

- [ ] 多报销模板支持
- [ ] 历史报销记录查询
- [ ] 更多发票类型支持
- [ ] OCR 模型热切换

---

## 8. 术语表

| 术语 | 说明 |
|------|------|
| OCR | 光学字符识别（Optical Character Recognition） |
| PaddleOCR | 百度开源 OCR 工具包 |
| MNN | 阿里轻量级神经网络推理引擎 |
| 一对多匹配 | 1 张发票对应多笔支付（打车场景） |
| 行程单 | 打车平台出具的行程明细单据 |
| 对照 PDF | 发票与支付记录的对照材料 |
