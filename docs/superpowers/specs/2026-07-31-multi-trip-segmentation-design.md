# 多趟出差自动分趟 & 分别导出 — 设计规格

> 创建日期: 2026-07-31
> 状态: 设计中

## 概述

现在发票解析已能从火车票/机票提取出发城市、到达城市和出行日期。本功能允许一次导入多趟出差的全部发票，根据票据的链式行程自动分组为多笔出差单，用户可手动调整分组后**每趟分别生成**报销文件（HTML/Excel/对照PDF/对照XLSX），无需用户手动为每一趟单独导入。

## 1. 分组算法（后端 Rust）

新增命令 `segment_trips`，输入 `Vec<MatchResult>`（已匹配的发票+账单），输出分组结果。纯函数、可单测。

### 1.1 票据筛选

- 仅 `category ∈ {Train, Flight}` 的发票视为票据
- 必须同时具备 `travel_date`、`departure_city`、`arrival_city`，否则视为普通发票（只能按日期归入窗口）

### 1.2 链式贪心成组

所有票据按 `travel_date` 升序排序后，贪心构建"出差链"：

1. 取最早未分配票据作为该趟**第 1 程**，记 `origin` = 它的出发城市，`frontier` = 它的到达城市
2. 依次考察后续未分配票据 `t`（按日期）：
   - **返程**：`t.出发 == frontier` 且 `t.到达 == origin` → 该趟完整。该趟 = 链上所有票据 + t。`travel_start` = 第 1 程日期，`travel_end` = t 的日期，`destination` = `frontier`（返程票出发的城市，即最后一程到达城市）
   - **续链**：`t.出发 == frontier` → 加入链，`frontier` = t 的到达城市，继续
   - **截断**：两者皆不满足 → 当前链**未凑齐返程**，链上全部票据进入"待调整"；t 作为**下一趟**的第 1 程重新开始（若后续仍凑不成完整往返，最终同样进"待调整"）
3. 所有票据处理完毕后的剩余链（有去无回）→ 进"待调整"

示例（长沙→武汉→北京→长沙）：`origin=长沙`，`frontier=武汉`→`北京`，返程 `北京→长沙` 到达 `origin` 且出发 == `frontier` → 一趟完整，destination=北京。

### 1.3 非票据发票归入

非票据发票按日期归入**完整趟**的窗口 `[travel_start, travel_end]`（含边界）：

| 类别 | 取用日期 |
|---|---|
| Hotel | `hotel_detail.check_in`（无则开票日） |
| CityTransport | 首个 `itinerary.date_time` 的日期（无行程则开票日） |
| Toll | `toll_travel_time` 的日期（无则开票日） |
| 其他 | 开票日 `date` |

- 同时落在多个窗口 → 归入最早开始的趟
- 不在任何窗口 → 进"待调整"
- **窗口内的其他票据**（非该趟配对票据）→ 一律进"待调整"，由用户决定归属（避免自动吞并相邻出差）

### 1.4 输出结构

```rust
pub struct TripGroup {
    pub id: String,              // 趟 id（如 "trip-1"）
    pub destination: String,     // 去程链最后一程到达城市
    pub travel_start: String,    // YYYY-MM-DD
    pub travel_end: String,      // YYYY-MM-DD
    pub ticket_ids: Vec<String>, // 该趟链上票据的 invoice_id（含返程）
    pub invoice_ids: Vec<String>,// 该趟全部发票的 invoice_id
}
pub struct SegmentResult {
    pub trips: Vec<TripGroup>,
    pub unassigned_ids: Vec<String>, // 待调整的 invoice_id
}
```

仅返回 id 引用，不拷贝发票数据；前端按 id 映射到 `matches`。

## 2. 前端状态（`src/stores/match.ts`）

- 新增响应式状态：
  - `trips: Trip[]` — 每趟含 `id`、`destination`、`travelStart`、`travelEnd`、`matches: MatchResult[]`、`ticketIds: string[]`
  - `unassigned: MatchResult[]` — 待调整发票
- `autoMatch` 成功后自动调用 `segment_trips` 重建分组（覆盖手动调整）
- 新增 `reSegmentTrips()` 供"重新自动分趟"按钮手动触发
- 手动调整（移动发票/新建出差）只修改 `trips`/`unassigned` 引用，不重算

## 3. 导出页 UI（`src/views/ExportView.vue` 改造）

### 3.1 分趟区

每趟一张卡片：

- 头部：目的地、起止日期、票据数、发票数、合计金额（该趟 matches 汇总）
- 复用现有 `ReimbursementForm`（destination/travelStart/travelEnd 预填，可改）
- 发票明细列表（默认折叠）：每张发票一行，含"移出"（→ 待调整）与"移到别的趟"下拉
- 复用现有 `ExportButton`：传入该趟的 `matchResults` 子集 + 该趟 formInfo → 每趟分别生成 HTML/XLSX/对照PDF/对照XLSX

### 3.2 待调整区

- 未分组发票列表，每张有下拉"移到出差 N"；若为票据，附"新建出差"按钮（以该票为去程建一趟，destination=其到达城市，起止=其出行日期）
- 提示文案：说明该区为无法自动配对/超出行程日期的票据与发票

### 3.3 兜底

- 无任何票据（无 Train/Flight 发票）→ 全部 matches 作为单趟显示，destination/travelStart/travelEnd 留空由用户手填，行为与现状一致
- 删除原"🎫 从票据提取"按钮与 `onMounted` 自动提取逻辑，由分趟结果替代

## 4. 导出复用

- 后端导出命令（`generate_reimbursement_html`/`render_reimbursement_html`/`generate_reimbursement_xlsx`/`generate_comparison_*`）**不改**：均接收 `match_results` + 表单信息，分趟导出只需传该趟子集与表单值
- 每趟导出文件命名沿用现有 `报销单_日期`、`对照表含图片_日期` 等格式

## 5. 测试

Rust 单元测试（`segment_trips`）：
1. 简单往返（A→B, B→A）→ 一趟
2. 链式往返（A→B, B→C, C→A）→ 一趟，destination=C
3. 多趟连续（往返1 + 往返2）→ 两趟
4. 单程票（A→B 无返程）→ 待调整
5. 链断（A→B, C→A 无中间程）→ 两张均待调整，C→A 开启新链
6. 非票据发票按日期落入窗口；跨窗口取最早；窗口外进待调整
7. 窗口内其他票据（非配对）进待调整

## 6. 边界与已知限制

- 中转票/多程票只要链连续即可自动归入；链断开（缺中间程）则进"待调整"由用户手动移动
- 跨趟日期重叠时，贪心取最早反向票，先到先得
- "重新自动分趟"会覆盖手动调整结果与手改的目的地/起止日期
- 手动移动发票后，该趟 destination/travelStart/travelEnd **不**自动重算（保留用户手改值）；仅重新分趟时重算
