# 票据城市 & 日期自动提取 — 设计规格

> 创建日期: 2026-06-18
> 状态: 设计中

## 概述

从火车票、机票中自动提取出发/到达城市信息，并在导出界面提供「从票据提取」按钮，一键填充报销表单的目的地和出差起止日期。

## 1. 数据模型

### Invoice 结构体 (`src-tauri/src/models/invoice.rs`)

新增两个可选字段：

```rust
pub struct Invoice {
    // ... 现有字段不变 ...
    pub departure_city: Option<String>,  // 出发城市（火车票发站/机票出发机场归一化）
    pub arrival_city: Option<String>,    // 到达城市（火车票到站/机票到达机场归一化）
}
```

- `None` 表示非火车/机票类别，或解析失败
- 向前兼容，不影响现有序列化

### 前端类型 (`src/types/invoice.ts`)

同步新增：

```typescript
export interface Invoice {
  // ... 现有字段 ...
  departureCity?: string;
  arrivalCity?: string;
}
```

## 2. 解析器

### 新增函数 (`src-tauri/src/parser/invoice_parser.rs`)

```rust
/// 从 OCR 文本提取火车票/机票的出发和到达城市
/// 返回 (departure_city, arrival_city)
fn extract_ticket_cities(ocr_text: &str, category: &InvoiceCategory) -> (Option<String>, Option<String>);
```

**火车票** 关键词匹配：
- 出发：`出发站[：:]?(\S{2,6}站?)` / `发站[：:]?(\S{2,6})`
- 到达：`到达站[：:]?(\S{2,6}站?)` / `到站[：:]?(\S{2,6})`

**机票** 关键词匹配：
- 出发：`自[：:]?\s*(\S{2,6}(?:机场|国际机场)?)` / `FROM[：:]?\s*(\S+)`
- 到达：`至[：:]?\s*(\S{2,6}(?:机场|国际机场)?)` / `TO[：:]?\s*(\S+)`

### 城市名归一化 (`station_to_city`)

```rust
fn station_to_city(raw: &str) -> String;
```

- 去除后缀：`站`、`东站`、`西站`、`南站`、`北站`、`机场`、`国际机场`
- 去除机场三字码：`PEK`、`SHA`、`PVG`、`CAN` 等
- 复用现有 `hotel_standard.rs:177` 的 `extract_city_keyword()` 逻辑
- 内置兜底映射表：
  - `虹桥` → `上海`
  - `宝安` → `深圳`
  - `江北` → `重庆`
  - `流亭` → `青岛`
  - `龙嘉` → `长春`
  - `太平` → `哈尔滨`
  - `遥墙` → `济南`
  - `周水子` → `大连`
  - 更多按需补充

### 回退策略

若关键词匹配失败，取整个文本中出现的第一个和最后一个 2-3 字城市名作为 fallback（参考 `extract_city_keyword` 的城市名识别逻辑）。

## 3. 流水线集成

### 调用点 (`src-tauri/src/pdf/invoice_pipeline.rs`)

在 `parse_invoice_from_pdf()` 和 `parse_invoice_from_image()` 中，当 invoice 解析成功后：

```
if invoice.category == Train || invoice.category == Flight {
    let (dep, arr) = extract_ticket_cities(&ocr_text, &invoice.category);
    invoice.departure_city = dep;
    invoice.arrival_city = arr;
}
```

**注意**：`ocr_text` 需要在此处可用。如果发票使用了 parangi 纯文本路径而非 OCR，需确保仍然有 OCR 文本（可通过强制 OCR 或使用 parangi 文本）。当前 pipeline 在 seller 为空时会回退到 OCR，可复用该分支的 OCR 文本。

### 改动文件清单

| 文件 | 改动 |
|---|---|
| `src-tauri/src/models/invoice.rs` | `Invoice` 加两个字段 |
| `src-tauri/src/parser/invoice_parser.rs` | 新增 `extract_ticket_cities()` + `station_to_city()` |
| `src-tauri/src/pdf/invoice_pipeline.rs` | 解析成功后插入城市提取调用 |
| `src/types/invoice.ts` | 同步前端类型 |

不改动：
- `invoice_type_detector.rs` — 已有 TrainInvoice/FlightInvoice 判定
- `field_extractors.rs` — 城市提取是独立的 OCR 文本分析
- `models/payment.rs` / `models/match_result.rs` / `models/reimbursement.rs`

## 4. 前端交互

### ExportView.vue 改动

新增 **「从票据提取」按钮** 在 ReimbursementForm 上方：

```
┌─────────────────────────────────┐
│  [🎫 从票据提取]                │
├─────────────────────────────────┤
│  到达城市:  [____________]      │
│  开始日期:  [____年__月__日]    │
│  结束日期:  [____年__月__日]    │
│  住宿级别:  [▼ 五星级  ]       │
└─────────────────────────────────┘
```

### 提取函数 `extractTripFromInvoices()`

```typescript
function extractTripFromInvoices() {
  // 1. 过滤 Ticket 类发票
  const tickets = invoices.value.filter(
    inv => (inv.category === 'Train' || inv.category === 'Flight') && inv.arrivalCity
  );
  if (tickets.length === 0) {
    toast.warning('未找到可提取的火车票或机票');
    return;
  }

  // 2. 按日期排序
  tickets.sort((a, b) => a.date.localeCompare(b.date));

  // 3. 目的地 = 最早一张票的到达城市
  formInfo.value.destination = tickets[0].arrivalCity!;

  // 4. 日期范围 = min/max
  formInfo.value.travelStart = tickets[0].date;
  formInfo.value.travelEnd = tickets[tickets.length - 1].date;
}
```

### 行为规则

| 场景 | 行为 |
|---|---|
| 无火车票/机票 | toast 提示「未找到可提取的火车票或机票」 |
| 有票但城市提取失败（arrivalCity 为空） | 跳过该票，不影响其他票的提取 |
| 多次点击 | 每次用最新发票列表重新填充（覆盖当前值） |
| 用户手动修改后 | 不受影响，除非再次点击按钮 |
| 只有一张票 | start = end = 该票日期 |

### 不改动

- `ReimbursementForm.vue` — 保持纯展示，不做任何修改

## 5. 错误处理

- OCR 文本质量差导致关键词匹配失败 → 回退到全文城市名扫描
- 城市名归一化失败 → 保留原始 OCR 文本（用户可手动修正）
- 无 `ocr_text` 可用 → departure_city/arrival_city 为 None，前端按钮跳过该票
- 前端日期排序依赖 `invoice.date` 字段（NaiveDate），格式一致，不需要额外处理

## 6. 测试策略

### 单元测试（Rust）

- `extract_ticket_cities()` 对典型火车票 OCR 文本的解析
- `extract_ticket_cities()` 对典型机票 OCR 文本的解析
- `station_to_city()` 对各类站名/机场名的归一化
- 边界：空文本、无关键词文本、乱码文本

### 集成测试（Rust）

- 完整 pipeline 对样本火车票 PDF 的处理，验证 departure_city/arrival_city 是否正确
- 完整 pipeline 对样本机票 PDF 的处理

### 手动测试

- 导入真实火车票 → 导出页面点击「从票据提取」→ 验证目的地和日期
- 导入真实机票 → 同上
- 导入混合票据（火车+机票）→ 验证日期范围正确
- 无票据时点击按钮 → 验证 toast 提示
