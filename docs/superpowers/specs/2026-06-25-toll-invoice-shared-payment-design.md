# 高速费发票共享支付匹配设计

日期：2026-06-25
状态：已批准

## 背景与问题

### 业务场景

出差打车时，一笔支付可能同时包含行程费和高速通行费。用户报销时需要贴两张发票：

- 行程发票（滴滴电子发票，含行程单）
- 高速费发票（ETC 电子发票或收费站纸质发票）

两张发票共用同一笔支付记录。

### 当前系统的局限

1. **支付独占模型**：`MatchResult` 的 `used_payment_ids` 占用支付后，其他发票不能复用。高速费发票无法匹配到已被行程占用的支付。
2. **金额不匹配**：高速费发票金额（如 10 元）与支付总额（如 60 元）相差过大，单独匹配必然失败。
3. **无 Toll 类别**：`InvoiceCategory` 没有"高速通行费"类别，高速费发票被误判为 `Other` 或 `CityTransport`。
4. **无通行时间字段**：高速费发票的开票日期因 ETC 延迟可能与实际通行时间差几天，无法用开票日期关联行程。

### 用户需求

- 高速费发票在报销单里视为一笔独立发票，与普通发票展示方式一致（发票图片在上，支付单号在下）。
- 支付单号允许在报销单中重复出现（行程发票和高速费发票各显示一次同一支付单号）。
- 自动关联高速费到行程，用户可手动调整。
- 匹配逻辑：先用通行时间关联高速费到行程，再用"行程金额 + 高速费金额"组合去匹配支付；一条行程匹配不上则换下一条行程尝试。

## 方案选择

### 方案B：高速费有独立 MatchResult，共享支付引用（已选）

每张高速费发票拥有独立的 `MatchResult`，通过 `shared_from_invoice_id` 指向行程发票，引用同一笔支付。`used_payment_ids` 只在行程匹配时占用一次，高速费复用不重复占用。

### 未选方案

- **方案A（高速费作为行程附属）**：高速费挂到行程 `MatchResult.linked_invoices`。改动最小，但高速费无独立 MatchResult，前端展示和手动调整需额外逻辑。未选，因为用户明确要求高速费在报销单里是独立发票。
- **方案C（发票组概念）**：新增 `InvoiceGroup` 打包行程+高速费。语义最完整但过度设计，当前只有一种组合需求。

## 数据模型变更

### `InvoiceCategory` 新增 Toll

```rust
pub enum InvoiceCategory {
    Train,
    Flight,
    TicketChange,
    CityTransport,
    Hotel,
    Meal,
    Toll,    // 新增：高速通行费
    Other,
}
```

### `Invoice` 新增通行时间字段

```rust
pub struct Invoice {
    // ... 现有字段 ...
    #[serde(default)]
    pub toll_travel_time: Option<chrono::NaiveDateTime>,  // 新增：通行时间（从备注提取，仅 Toll 类）
}
```

- 仅 `Toll` 类别发票有值。
- 从 `remarks` 用正则提取，格式见下文。
- 旧数据 serde 反序列化时默认 `None`（`#[serde(default)]`）。

### `MatchResult` 新增共享字段

```rust
pub struct MatchResult {
    // ... 现有字段 ...
    #[serde(default)]
    pub shared_payment_ids: Vec<String>,           // 新增：标记共享的支付ID
    #[serde(default)]
    pub shared_from_invoice_id: Option<String>,    // 新增：共享来源发票ID（高速费指向行程）
}
```

- 行程发票的 `MatchResult`：`shared_payment_ids` 和 `shared_from_invoice_id` 均为空。
- 高速费发票的 `MatchResult`：`payments` 引用行程匹配到的同一笔支付，`shared_from_invoice_id` = 行程发票 ID，`shared_payment_ids` = 该支付 ID。

## 高速费识别

### 电子发票自动识别

在 `invoice_type_detector` 中按关键词识别 Toll 类别：

- 关键词：`通行费`、`高速`、`ETC`、`过路费`
- 命中关键词且金额特征符合（通常 < 100 元）→ `InvoiceCategory::Toll`

### 手动票据

用户在界面手动添加发票时选择 `Toll` 类别，录入金额和备注。

### 通行时间提取

从 `remarks` 提取通行时间，存入 `toll_travel_time`。

**备注格式示例**：

```
湘ADG5926 湖南新港站入 湖南黄花站出 2026-05-25 10:06:04 （不可用于增值税进项抵扣）
川AB55365 四川天府机场T1T2站入 四川天府机场成都站出 2026-06-23 14:24:10 （不可用于增值税进项抵扣）
```

**提取规则**：正则 `(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})`，取第一个匹配。提取失败时 `toll_travel_time` 为 `None`，回退到开票日期 `invoice.date`。

**入站/出站信息**：界面卡片展示用，从备注解析。规则：正则 `(\S+站)入` 提取入站，`(\S+站)出` 提取出站。PDF 不展示。

## 匹配流程

### `batch_match` 改造

```
1. 分离 Toll 发票和 CityTransport 发票（含行程单的）
2. 自动关联：每个 Toll 按 toll_travel_time 找时间最近的 CityTransport
   - 默认一对一关联
   - toll_travel_time 为 None 时回退用 invoice.date
   - 无 CityTransport 可关联时，Toll 发票标记为未匹配
3. 匹配 CityTransport 时：
   - 若有关联的 Toll，目标金额 = 行程金额 + Toll 金额
   - 用组合金额匹配支付（现有行程匹配/组合匹配逻辑）
   - 匹配不上时，换下一条行程尝试（解除当前 Toll 关联，尝试关联到下一个 CityTransport）
4. 匹配成功后：
   - 行程发票 MatchResult：payments = [匹配的支付]，shared 字段为空
   - 高速费 MatchResult：payments = [同一笔支付]，shared_from_invoice_id = 行程发票ID，shared_payment_ids = [支付ID]
   - 该支付ID加入 used_payment_ids（只加一次）
5. 未关联到任何行程的 Toll 发票 → unmatched_invoices
```

### 自动关联的时间规则

- Toll 的 `toll_travel_time` 与 CityTransport 首条行程时间 `itineraries[0].date_time` 比较
- 时间差绝对值最小者关联
- 无时间上限硬约束（ETC 开票延迟、通行时间与行程时间可能差几小时），但优先选最近的

### 共享支付的 `used_payment_ids` 逻辑

- 行程匹配占用支付 → `used_payment_ids.push(支付ID)`
- 高速费 MatchResult 引用同一支付 → **不再 push**（避免重复占用）
- `unmatched_payments` 计算：`payments` 中不在 `used_payment_ids` 的记录。高速费共享的支付已被行程占用，不会出现在未匹配列表。

## 报销单与界面展示

### PDF 生成

高速费发票与普通发票展示方式一致：

- 发票图片在上
- 支付单号在下（允许重复，行程发票和高速费发票各显示一次同一支付单号）
- 不展示入站/出站信息

### 界面匹配卡片

高速费卡片额外展示：

- 入站/出站信息（从备注解析"XX站入""XX站出"）
- "共享自 [行程发票号]" 标记，表明支付来自行程发票

## 手动调整

- 用户可在界面把高速费从一张行程关联到另一张行程
- 调整后重新计算组合金额并重新匹配支付
- 用户可解除关联（高速费变为未匹配）
- 用户可手动为高速费指定支付记录（手动匹配，标记为共享）

## 测试策略

### 单元测试

1. **Toll 类别识别**：电子发票关键词命中 → Toll；手动票据用户选 Toll。
2. **通行时间提取**：正则从备注提取 `YYYY-MM-DD HH:MM:SS`；无匹配时回退 None。
3. **自动关联**：
   - Toll 按通行时间关联到最近的 CityTransport
   - toll_travel_time 为 None 时回退用 invoice.date
   - 无 CityTransport 时 Toll 标记未匹配
4. **组合金额匹配**：
   - 行程 50 + 高速费 10 = 60，匹配到 60 元支付
   - 第一条行程匹配不上时换下一条行程
5. **共享支付**：
   - 行程 MatchResult 占用支付，高速费 MatchResult 引用同一支付
   - `used_payment_ids` 只含一次该支付ID
   - `unmatched_payments` 不含该支付
6. **报销单生成**：高速费发票独立行，支付单号与行程重复显示。

### 集成测试

端到端：导入含行程发票 + 高速费发票 + 共享支付的账单，验证两张发票都匹配成功且共用支付。

## 影响范围

### 需修改的文件

- `src-tauri/src/models/invoice.rs`：新增 `Toll` 类别、`toll_travel_time` 字段
- `src-tauri/src/models/match_result.rs`：新增 `shared_payment_ids`、`shared_from_invoice_id` 字段
- `src-tauri/src/parser/invoice_type_detector.rs`：Toll 关键词识别
- `src-tauri/src/matching/batch.rs`：Toll 分离、自动关联、组合金额匹配、共享支付逻辑
- `src-tauri/src/matching/manual.rs`：手动匹配高速费时支持标记共享支付
- `src-tauri/src/pdf/form_xlsx_generator.rs`：高速费发票行展示（支付单号允许重复）
- `src-tauri/src/pdf/comparison_xlsx_generator.rs`：同上
- `src-tauri/src/pdf/comparison_image_pdf_generator.rs`：同上
- 前端 Vue 组件：Toll 类别选项、高速费卡片展示入站/出站、手动关联 UI

### 向后兼容

- `toll_travel_time`、`shared_payment_ids`、`shared_from_invoice_id` 均加 `#[serde(default)]`，旧数据反序列化不报错。
- 现有非 Toll 发票的匹配逻辑不受影响。
