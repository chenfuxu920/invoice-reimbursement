# 统一 datetime 提取/解析模块设计

日期：2026-08-01

## 背景与动机

滴滴行程单第 2 页的时间泄漏周几问题（`05-11 11:48 周\n一`）暴露了当前实现的缺陷：
解析时间采用"先剥离噪声（周几）→ 再清洗文本"的思路。但换行位置不可信任——
周几、分钟等可能被换行拆成任意形态，剥噪正则（精确匹配 `周一` 等）匹配不到拆分形态就泄漏。

更深层的问题：**时间/日期的正则匹配散落在 7 处**，各自为政，格式列表不统一：

| 位置 | 现状 |
|------|------|
| `itinerary_parser.rs:552` | `re_weekday` 剥周几 + `re_colon_space` 修冒号空格 |
| `itinerary_parser.rs:288-295` | `parse_table_generic` 的 8 个 `re_datetime_*` 正则链 |
| `itinerary_parser.rs:1383` | `extract_reference_times_ordered` 交叉验证时间提取 |
| `matching/batch.rs:707` | `parse_datetime` chrono 格式列表 |
| `matching/engine.rs:12` | `parse_payment_date` chrono 格式列表 |
| `invoice_parser.rs:1061` | `extract_toll_travel_time` 高速费通行时间 |
| `comparison_xlsx_generator.rs:512` | `compute_time_diff` 手动格式列表 |

## 设计原则

1. **直接提取而非先污染后清洗**：用格式正则列表直接从噪杂文本提取 datetime，
   周几/换行/序号等噪声天然被正则忽略（正则只捕获数字部分），不需要先剥周几。
2. **两层 API**：
   - Layer 1（提取）：噪杂文本 → 规范化字符串
   - Layer 2（解析）：规范化字符串 → `NaiveDateTime`（兼容无年份）
   行程单需要保留无年份 `MM-DD HH:MM` 形态（后续 `enrich_itinerary_years` 补年），
   所以不能统一返回 `NaiveDateTime`，分两层各取所需。
3. **统一管理**：所有 datetime 匹配站点收敛到单一模块，格式列表只维护一处。

## 新模块

新增 `src/parser/datetime_util.rs`，在 `parser/mod.rs` re-export。

### Layer 1：`extract_datetime(text: &str) -> Option<String>`

从噪杂文本提取规范化 datetime 字符串。有序尝试以下格式正则（越靠前越具体，命中即返回）：

| # | 格式 | 示例 | 输出 |
|---|------|------|------|
| 1 | `YYYY-MM-DD HH:MM:SS` | `2026-04-24 17:58:59` | `2026-04-24 17:58:59` |
| 2 | `YYYY-MM-DD HH:MM`（日期时间间空格可选） | `2026-04-2408:48` | `2026-04-24 08:48` |
| 3 | `YYYY[-/年]MM[-/月]DD`（纯日期） | `2026/04/24` | `2026-04-24` |
| 4 | `MM-DD HH:MM`（组件间容忍 `\s*`，含 `:` 后换行） | `05-11 11:48 周\n一`、`04-22 21:\n10 周三` | `05-11 11:48` |
| 5 | `MM-DD HH:??`（分钟缺失哨兵） | `07-0320`、`07-03 20:` | `07-03 20:??` |
| 6 | `MM-DD`（纯日期，无时间） | `04-28` | `04-28` |

关键点：
- 周几、换行、序号等噪声天然被正则忽略，不再需要 `re_weekday`/`re_colon_space`。
- 模式 5 保留 `:??` 哨兵：`parse_itinerary_from_tables` 用它判断不完整行程，
  触发交叉验证恢复（现有 `itinerary_is_incomplete` / `has_incomplete_entries` 逻辑不变）。
- 模式 6 保留纯日期提取（`extract_date` 场景用）。

### Layer 2：`parse_datetime(s: &str) -> Option<NaiveDateTime>`

把规范化字符串（或外部传入的 datetime 字符串）解析为 `NaiveDateTime`：
- chrono 格式列表：`%Y-%m-%d %H:%M:%S`、`%Y-%m-%d %H:%M`、`%Y-%m-%d`、
  `%Y/%m/%d %H:%M:%S`、`%Y/%m/%d %H:%M`、`%Y/%m/%d`、无空格 `%Y-%m-%d%H:%M:%S`、`%Y-%m-%d%H:%M`
- 无年份 `MM-DD HH:MM`：按当年/去年解析（沿用 batch.rs:707 现有逻辑）
- 含 `:??` 视为 None（不完整）

### Layer 2b：`extract_date(s: &str) -> Option<String>`

从 datetime 字符串提取 `YYYY-MM-DD` 日期部分：
- `YYYY-MM-DD HH:MM:SS` → 截前 10 位
- `MM-DD HH:` 无年份 → 补当年 `YYYY-MM-DD`（沿用 batch.rs:777 现有逻辑）
- Excel 序列号 → 转日期（沿用 batch.rs:777 现有逻辑）

## 替换清单（7 处）

| 站点 | 现状 | 改为 |
|------|------|------|
| `itinerary_parser.rs:662` `parse_itinerary_from_tables` | `re_weekday`+`re_colon_space` 剥周几 | `extract_datetime(&cell.line_text)` |
| `itinerary_parser.rs:407-434` `parse_table_generic` | 8 个 `re_datetime_*` 正则链 | `extract_datetime(&time_text)`（保留 `:??` 哨兵语义） |
| `itinerary_parser.rs:1383` `extract_reference_times_ordered` | 独立 `re_main`/`re_cont_min` | 复用 Layer 1/2（保留序号锚定结构） |
| `matching/batch.rs:707` `parse_datetime` | 本地 chrono 列表 | Layer 2 |
| `matching/engine.rs:12` `parse_payment_date` | 本地 chrono 列表 | Layer 2（取 `.date()`） |
| `invoice_parser.rs:1061` `extract_toll_travel_time` | 本地正则 | `extract_datetime` → Layer 2 |
| `comparison_xlsx_generator.rs:512` `compute_time_diff` | 手动格式列表 | Layer 2 |

约束：
- 每个站点替换后行为与现状一致（无回归），改动只收敛实现、不改语义。
- `matching/batch.rs` 的 `extract_date` 与 `parse_datetime` 由 Layer 2/2b 接管；
  站点内保留 Excel 序列号特殊逻辑（wechat_parser 的序列号转换是数据源层，不在本次范围）。
- `itinerary_parser` 的 `merge_split_times`（换行分钟合并）属于"行结构合并"，
  在 Layer 1 之外、保留在站点内（它处理的是跨行结构而非格式识别）。

## 测试

### 新增单测（datetime_util 模块内）
- 6 种格式逐个提取
- 周几跨行：`05-11 11:48 周\n一` → `05-11 11:48`
- 换行分钟：`04-22 21:\n10 周三` → `04-22 21:10`
- 无年份解析：`04-22 21:10` → 当年 `NaiveDateTime`
- `:??` 解析 → None
- Excel 序列号 → 日期

### 回归
- `pdfplumber_cell_debug_test`（行程单表格单元格提取）
- `pdfplumber_pipeline_test`（滴滴 A/B、天府通、发票全流程）
- `itinerary_parser` 模块单测
- `matching/batch`、`matching/engine` 单测

## 已知范围外

- `wechat_parser` 的 Excel 序列号转字符串（数据源层，非匹配逻辑）
- `text_extractor.rs` 的 char 级时间恢复（火车票 `reconstruct_lines_from_chars`）
- 日期格式的展示层（`comparison_xlsx_generator` 的格式串输出）
