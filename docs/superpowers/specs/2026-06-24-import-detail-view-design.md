# 导入界面发票/账单详情查看 设计文档

- 日期：2026-06-24
- 状态：已批准，待实现
- 关联视图：`ImportView.vue`（`/import`）

## 1. 背景与目标

当前导入界面（`ImportView.vue`）仅以 `InvoiceCard` 展示发票摘要（金额/销售方/发票号/日期），以 `PaymentTable` 展示账单表格行。发票详情弹窗（`InvoiceDetailModal`，含文件预览 + 行程明细）和支付详情弹窗（`PaymentDetailModal`）目前只在匹配页（`MatchView`）使用。

**目标**：将详情查看能力前移到导入界面，使用户在导入阶段即可：
- 查看发票文件预览及全部详情
- 查看账单条目的全部字段
- 对解析失败的发票，对照文件缩略图手动填写发票信息

## 2. 现状

| 组件 | 文件 | 现状 |
|---|---|---|
| `ImportView.vue` | `src/views/ImportView.vue` | 发票上传 + 账单导入 + 全局导入 |
| `InvoiceCard.vue` | `src/components/InvoiceCard.vue` | 摘要卡片，含删除按钮 |
| `InvoiceDropZone.vue` | `src/components/InvoiceDropZone.vue` | 拖拽/选择发票文件 |
| `PaymentTable.vue` | `src/components/PaymentTable.vue` | 账单表格，含删除按钮 |
| `BillImporter.vue` | `src/components/BillImporter.vue` | 微信/支付宝账单导入 |
| `InvoiceDetailModal.vue` | `src/components/InvoiceDetailModal.vue` | 发票详情弹窗（预览+行程），仅 MatchView 使用 |
| `PaymentDetailModal.vue` | `src/components/PaymentDetailModal.vue` | 支付详情弹窗，仅 MatchView 使用 |

后端命令（`src-tauri/src/lib.rs`）：
- `recognize_invoice` / `batch_recognize` → 返回 `{ invoices, errors, duplicates }`
- `batch_global_import` → 返回 `{ invoices, payments, errors, duplicates }`
- `render_pdf_preview(file_path)` → `Vec<String>`（base64 图片，支持多页）
- `open_file_with_system(file_path)` → `()`

数据模型：
- `Invoice`（`src-tauri/src/models/invoice.rs`）：id, invoice_number, amount, seller_name, item_name, date, travel_date, category, source, itineraries, itinerary_file, remarks, hotel_detail, departure_city, arrival_city
- `Itinerary`：date_time, provider, pickup, dropoff, amount
- `PaymentRecord`（`src-tauri/src/models/payment.rs`）：id, transaction_id, transaction_time, amount, original_amount, refund_amount, discount, merchant_name, source, category, payment_method

## 3. 设计方案

### 3.1 交互模式

采用"内联展开摘要 + 弹窗看完整详情"组合模式（方案 C）。

- 发票卡片：整卡点击切换内联展开；标题/金额区可点打开详情弹窗
- 账单行：整行点击切换内联展开；无弹窗（账单无文件预览意义，展开已含全部字段）
- 解析失败项：独立错误区，点击条目打开手动填写弹窗

### 3.2 发票卡片改造（InvoiceCard.vue）

**交互**：
- 整卡点击 → 切换内联展开/收起，右侧 ▾/▸ 箭头指示状态
- 卡片顶部标题/金额区 → 独立可点击（可点链接样式提示），点击 emit `view-detail`，由父组件打开 `InvoiceDetailModal`
- 两个动作入口明确分离：展开箭头/卡片主体 vs 标题区链接

**内联摘要内容**（展开后）：
- 发票类别
- 商品/服务名称（item_name）
- 来源文件名（source 或 itinerary_file）
- 行程明细文本列表：每段一行 `时间 | 平台 | 起点→终点 | 金额`
- 无行程时显示"无行程明细"

**Props/Emits 变更**：
- 新增 emit `view-detail`（payload: Invoice）
- 新增内部状态 `expanded: boolean`

### 3.3 账单行改造（PaymentTable.vue）

**交互**：
- 整行点击 → 切换展开下方详情行
- 无弹窗

**展开内容**（`PaymentRecord` 全部字段）：
- 交易单号（transaction_id）
- 交易时间（transaction_time）
- 实付金额（amount）
- 原始金额（original_amount）
- 退款金额（refund_amount）
- 优惠金额（discount）
- 商户名称（merchant_name）
- 来源（source：Wechat/Alipay）
- 交易类型（category）
- 支付方式（payment_method）

### 3.4 错误区（ImportView.vue 新增）

**位置**：发票卡片列表下方，独立区块，标题"解析失败（N）"。

**数据来源**：`batch_recognize` 和 `batch_global_import` 返回的 `errors` 字段，存入 invoice store 的 `parseErrors`。

**每条错误展示**：
- 文件名
- 错误原因（message）
- 操作按钮：重试 / 手动填写 / 移除

**点击错误条目**（或"手动填写"按钮）→ 打开 `ManualInvoiceEntryModal`，传入文件路径。

**空状态**：无错误时不显示该区块。

### 3.5 手动填写弹窗（ManualInvoiceEntryModal.vue 新建）

**布局**：左侧缩略图 + 右侧表单。

**左侧缩略图**：
- 调用 `render_pdf_preview(filePath)` 渲染 PDF 第一页（及翻页）
- 图片文件（jpg/png）直接显示
- 加载中显示占位符

**右侧表单**：
- 核心字段：
  - 发票号（invoice_number，文本）
  - 金额（amount，数字）
  - 销售方（seller_name，文本）
  - 商品/服务（item_name，文本）
  - 开票日期（date，日期选择）
  - 类别（category，下拉：与 InvoiceCategory 枚举一致）
  - 来源（source，下拉：与 InvoiceSource 枚举一致）
- 可展开"行程明细"录入区：
  - 每条：时间（date_time）、平台（provider）、起点（pickup）、终点（dropoff）、金额（amount）
  - 支持新增/删除多条
  - 默认收起，用户判断为行程单时手动展开

**底部按钮**：保存 / 取消。

**保存行为**：
- 构造 `Invoice` 对象（生成 id），加入 invoice store
- 从错误区移除该条目（`removeParseError`）
- 关闭弹窗

### 3.6 发票详情弹窗（InvoiceDetailModal.vue 复用）

- 现有组件已具备：文件预览（`render_pdf_preview`）+ 行程明细 + 系统打开按钮
- 直接在 `ImportView.vue` 中接入，由 `InvoiceCard` 的 `view-detail` 事件触发
- 无需改动组件本身

## 4. 数据流

```
batch_recognize / batch_global_import
  ├─ invoices  → invoiceStore.invoices   → InvoiceCard 列表
  ├─ errors    → invoiceStore.parseErrors → 错误区
  └─ payments  → paymentStore.payments    → PaymentTable

InvoiceCard 标题区点击 → emit view-detail → ImportView 打开 InvoiceDetailModal
  └─ render_pdf_preview → 文件预览 + 行程明细

错误条目点击 → ManualInvoiceEntryModal(filePath)
  ├─ render_pdf_preview → 左侧缩略图
  └─ 保存 → invoiceStore.addManualInvoice(invoice) + removeParseError(id)

PaymentTable 行点击 → 内联展开全部字段（无弹窗）
```

## 5. 数据结构变更

### 5.1 新增类型（src/types/invoice.ts）

```typescript
export interface ParseError {
  id: string
  filePath: string
  fileName: string
  message: string
}
```

### 5.2 Store 改动（src/stores/invoice.ts）

- 新增 `parseErrors: Ref<ParseError[]>`
- 新增 `addParseErrors(errors: ParseError[])`：批量追加（导入后调用）
- 新增 `removeParseError(id: string)`：手动填写保存后或重试成功后调用
- 新增 `clearParseErrors()`：清空全部时调用
- 新增 `addManualInvoice(invoice: Invoice)`：手动填写的发票加入 `invoices` 列表
- `batch_recognize` / `batch_global_import` 调用后，将返回的 errors 转换为 `ParseError[]` 写入 store（而非仅 toast）

### 5.3 ImportView 改动

- 导入后：`invoiceStore.addParseErrors(mappedErrors)`
- 清空全部时：`invoiceStore.clearParseErrors()`
- 管理 `selectedInvoice`（弹窗）和 `manualEntryFile`（手动填写弹窗）两个状态

## 6. 范围边界（YAGNI）

- 账单不设弹窗（展开已含全部字段，无文件预览意义）
- 手动填写表单不含酒店详情、出发/到达城市、备注（核心字段 + 行程明细已覆盖主要场景，其余字段后续在匹配页补充）
- 缩略图仅在弹窗渲染（内联展开为纯文本，避免批量渲染性能问题）
- 错误区不做自动重试批量执行（仅单条重试按钮）
- 不做账单与发票的预匹配提示（匹配在 MatchView 进行）

## 7. 测试考虑

- `InvoiceCard`：展开/收起切换、标题区点击 emit 事件、行程明细渲染（有/无行程）
- `PaymentTable`：行展开/收起、全部字段渲染
- `ManualInvoiceEntryModal`：缩略图加载、表单校验、行程明细增删、保存后 store 更新 + 错误移除
- `ImportView`：错误区显示/隐藏、错误条目点击打开弹窗、清空时错误区清空
- Store：`parseErrors` 增删、`addManualInvoice` 加入列表

## 8. 涉及文件清单

| 文件 | 动作 |
|---|---|
| `src/components/InvoiceCard.vue` | 改造 |
| `src/components/PaymentTable.vue` | 改造 |
| `src/components/ManualInvoiceEntryModal.vue` | 新建 |
| `src/components/InvoiceDetailModal.vue` | 复用（不改） |
| `src/views/ImportView.vue` | 改造 |
| `src/stores/invoice.ts` | 改造 |
| `src/types/invoice.ts` | 新增 ParseError 类型 |
