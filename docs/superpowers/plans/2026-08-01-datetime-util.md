# 统一 datetime 提取/解析模块实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 新增 `datetime_util` 模块，统一散落在 7 处的 datetime 匹配逻辑；用"格式列表直接提取"替代"先剥周几再清洗"，修复滴滴第 2 页周几泄漏。

**架构：** 两层 API。Layer 1 `extract_datetime(text) -> Option<String>` 从噪杂文本按有序正则列表提取规范化时间串（周几/换行/序号天然被忽略）；Layer 2 `parse_datetime(s) -> Option<NaiveDateTime>` 把规范化串解析为时间（兼容无年份、`:??`）；Layer 2b `extract_date(s) -> Option<String>` 提取日期（含 Excel 序列号）。各站点收敛为薄委托或直接调用。

**技术栈：** Rust、regex、chrono、pdfplumber（feature-gated 测试）

---

## 范围调整说明（相对设计文档）

- `extract_reference_times_ordered`（itinerary_parser.rs:1383）**保留原结构**：它的 `re_main`/`re_cont_min` 是"序号锚定 + 续行分钟搜索"的结构提取，不是格式列表解析，输出已与 Layer 1 一致（`MM-DD HH:MM` / `:??`）。不纳入本次改造，仅回归确认。
- `invoice_parser.rs:1427` 的 `extract_date`（开票日期，处理 `2026年05月06日` 中文格式）**不在范围**：它是发票日期语义（返回 `NaiveDate` 且带默认回退），与 datetime 字符串匹配不同，保持现状。

## 文件结构

- 创建：`src-tauri/src/parser/datetime_util.rs` — Layer 1/2/2b + 单测
- 修改：`src-tauri/src/parser/mod.rs` — 添加 `pub mod datetime_util;`
- 修改：`src-tauri/src/parser/itinerary_parser.rs` — 2 处替换
- 修改：`src-tauri/src/parser/invoice_parser.rs:1061` — toll 时间替换
- 修改：`src-tauri/src/matching/batch.rs:707,777` — 委托 Layer 2/2b
- 修改：`src-tauri/src/matching/engine.rs:12` — 委托 Layer 2
- 修改：`src-tauri/src/matching/scoring.rs:240` — 委托 Layer 2
- 修改：`src-tauri/src/pdf/comparison_xlsx_generator.rs:512` — 委托 Layer 2

---

### 任务 1：创建 datetime_util 模块 + Layer 1 extract_datetime

**文件：**
- 创建：`src-tauri/src/parser/datetime_util.rs`
- 修改：`src-tauri/src/parser/mod.rs`

- [ ] **步骤 1：创建模块文件，写入 Layer 1 + 单测**

```rust
use chrono::NaiveDateTime;
use regex::Regex;
use std::sync::OnceLock;

/// 有序 datetime 格式（越靠前越具体，命中即返回）。每种格式的捕获组结构见 formatter。
enum Kind {
    FullSec,     // YYYY-MM-DD HH:MM:SS
    Full,        // YYYY-MM-DD HH:MM（日期时间可粘连）
    Date,        // YYYY[-/年]MM[-/月]DD
    Short,       // MM-DD HH:MM（组件间容忍 \s*，含冒号后换行）
    ShortMergedMin, // MM-DD HH MM（无冒号，空格分隔分钟）
    ShortIncomplete, // MM-DD HH:??（分钟缺失哨兵）
    ShortDate,   // MM-DD
}

struct Pat {
    re: &'static str,
    kind: Kind,
}

static PATTERNS: &[Pat] = &[
    Pat { re: r"(\d{4})[-/年](\d{1,2})[-/月](\d{1,2})[\s日]*(\d{1,2}):(\d{2}):(\d{2})", kind: Kind::FullSec },
    Pat { re: r"(\d{4})[-/年](\d{1,2})[-/月](\d{1,2})[\s日]*(\d{1,2}):(\d{2})", kind: Kind::Full },
    Pat { re: r"(\d{4})[-/年](\d{1,2})[-/月](\d{1,2})", kind: Kind::Date },
    Pat { re: r"(\d{2})-(\d{2})[\s]*(\d{1,2})[:：][\s]*(\d{2})", kind: Kind::Short },
    Pat { re: r"(\d{2})-(\d{2})(\d{1,2})[\s]+(\d{2})", kind: Kind::ShortMergedMin },
    Pat { re: r"(\d{2})-(\d{2})[\s]*(\d{1,2})[:：]", kind: Kind::ShortIncomplete },
    Pat { re: r"(\d{2})-(\d{2})", kind: Kind::ShortDate },
];

fn compiled() -> &'static Vec<(Regex, Kind)> {
    static RE: OnceLock<Vec<(Regex, Kind)>> = OnceLock::new();
    RE.get_or_init(|| PATTERNS.iter().map(|p| (Regex::new(p.re).unwrap(), p.kind)).collect())
}

fn pad2(s: &str) -> String {
    s.parse::<u32>().map_or_else(|_| s.to_string(), |n| format!("{n:02}"))
}

/// Layer 1：从噪杂文本提取规范化 datetime 字符串。
/// 周几、换行、序号等噪声天然被正则忽略（只捕获数字部分），无需先剥周几。
/// 输出形态：`YYYY-MM-DD HH:MM:SS` / `YYYY-MM-DD HH:MM` / `YYYY-MM-DD` /
///          `MM-DD HH:MM` / `MM-DD HH:??` / `MM-DD`
pub fn extract_datetime(text: &str) -> Option<String> {
    for (re, kind) in compiled() {
        if let Some(c) = re.captures(text) {
            return Some(match kind {
                Kind::FullSec => format!("{}-{}-{} {}:{}:{}", c[1], pad2(&c[2]), pad2(&c[3]), pad2(&c[4]), c[5], c[6]),
                Kind::Full => format!("{}-{}-{} {}:{}", c[1], pad2(&c[2]), pad2(&c[3]), pad2(&c[4]), c[5]),
                Kind::Date => format!("{}-{}-{}", c[1], pad2(&c[2]), pad2(&c[3])),
                Kind::Short => format!("{}-{} {}:{}", c[1], c[2], pad2(&c[3]), c[4]),
                Kind::ShortMergedMin => format!("{}-{} {}:{}", c[1], c[2], pad2(&c[3]), pad2(&c[4])),
                Kind::ShortIncomplete => format!("{}-{} {}:??", c[1], c[2], pad2(&c[3])),
                Kind::ShortDate => format!("{}-{}", c[1], c[2]),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_full_datetime() {
        assert_eq!(extract_datetime("2026-04-24 17:58:59").as_deref(), Some("2026-04-24 17:58:59"));
    }

    #[test]
    fn test_extract_full_no_space() {
        assert_eq!(extract_datetime("2026-04-2408:48:00").as_deref(), Some("2026-04-24 08:48:00"));
    }

    #[test]
    fn test_extract_full_no_seconds() {
        assert_eq!(extract_datetime("2026-04-24 08:48").as_deref(), Some("2026-04-24 08:48"));
    }

    #[test]
    fn test_extract_date_only_chinese_and_slash() {
        assert_eq!(extract_datetime("2026/04/24").as_deref(), Some("2026-04-24"));
        assert_eq!(extract_datetime("2026年04月24日").as_deref(), Some("2026-04-24"));
    }

    #[test]
    fn test_extract_short_weekday_split_newline() {
        assert_eq!(extract_datetime("05-11 11:48 周\n一").as_deref(), Some("05-11 11:48"));
    }

    #[test]
    fn test_extract_short_colon_newline_minutes() {
        assert_eq!(extract_datetime("04-22 21:\n10 周三").as_deref(), Some("04-22 21:10"));
    }

    #[test]
    fn test_extract_short_colon_space() {
        assert_eq!(extract_datetime("04-22 21: 10").as_deref(), Some("04-22 21:10"));
    }

    #[test]
    fn test_extract_short_incomplete_minutes() {
        assert_eq!(extract_datetime("07-03 20:").as_deref(), Some("07-03 20:??"));
    }

    #[test]
    fn test_extract_short_merged_incomplete() {
        assert_eq!(extract_datetime("07-0320").as_deref(), Some("07-03 20:??"));
    }

    #[test]
    fn test_extract_short_merged_minutes() {
        assert_eq!(extract_datetime("07-0320 46").as_deref(), Some("07-03 20:46"));
    }

    #[test]
    fn test_extract_short_date_only() {
        assert_eq!(extract_datetime("04-28").as_deref(), Some("04-28"));
    }

    #[test]
    fn test_extract_no_datetime_returns_none() {
        assert_eq!(extract_datetime("专车 成都"), None);
    }
}
```

- [ ] **步骤 2：注册模块并跑测试（预期失败：模块未声明）**

在 `src-tauri/src/parser/mod.rs` 顶部加入 `pub mod datetime_util;`

运行：`cargo test --lib parser::datetime_util --no-default-features`
预期：编译失败，报 `could not find `datetime_util` in `parser`` 或类似——若先跑实现则直接通过。若失败正常，下一步再写实现。

- [ ] **步骤 3：跑测试确认通过**

运行：`cargo test --lib parser::datetime_util`
预期：10 个单测全 PASS

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/parser/datetime_util.rs src-tauri/src/parser/mod.rs
git commit -m "feat: datetime_util 模块 Layer 1 extract_datetime（格式列表直接提取）"
```

---

### 任务 2：Layer 2 parse_datetime + Layer 2b extract_date

**文件：**
- 修改：`src-tauri/src/parser/datetime_util.rs`

- [ ] **步骤 1：追加 Layer 2/2b + 单测**

在 `extract_datetime` 之后、`#[cfg(test)]` 之前追加：

```rust
/// Layer 2：把规范化 datetime 字符串解析为 NaiveDateTime。
/// 支持：完整/斜杠/粘连格式、无年份 MM-DD（按当年/去年）、尾部冒号、`:??` 视为 None。
pub fn parse_datetime(s: &str) -> Option<NaiveDateTime> {
    let cleaned = s.trim().trim_end_matches(':').trim().to_string();
    if cleaned.contains("??") {
        return None;
    }
    const FORMATS: &[&str] = &[
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
        "%Y/%m/%d",
        "%Y-%m-%d%H:%M:%S",
        "%Y-%m-%d%H:%M",
    ];
    for fmt in FORMATS {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&cleaned, fmt) {
            return Some(dt);
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(&cleaned, fmt) {
            return d.and_hms_opt(0, 0, 0);
        }
    }
    // 无年份 MM-DD：按当年/去年解析
    if cleaned.len() >= 5
        && cleaned.as_bytes().get(2) == Some(&b'-')
        && cleaned[..2].chars().all(|c| c.is_ascii_digit())
        && cleaned[3..5].chars().all(|c| c.is_ascii_digit())
    {
        let current_year = chrono::Local::now().year();
        for year in [current_year, current_year - 1] {
            let with_year = format!("{}-{}", year, cleaned);
            for fmt in FORMATS {
                if let Ok(dt) = NaiveDateTime::parse_from_str(&with_year, fmt) {
                    return Some(dt);
                }
                if let Ok(d) = chrono::NaiveDate::parse_from_str(&with_year, fmt) {
                    return d.and_hms_opt(0, 0, 0);
                }
            }
        }
    }
    None
}

/// Layer 2b：从 datetime 字符串提取 YYYY-MM-DD 日期部分。
/// 支持：完整字符串、无年份 MM-DD（补当年）、Excel 序列号。
pub fn extract_date(s: &str) -> Option<String> {
    if s.len() >= 10 && s.as_bytes()[4] == b'-' {
        return Some(s[..10].to_string());
    }
    if s.len() >= 5 && s.as_bytes()[2] == b'-' {
        let mmdd = &s[..5];
        if mmdd.bytes().all(|c| c.is_ascii_digit() || c == b'-') {
            let year = chrono::Local::now().year();
            return Some(format!("{}-{}", year, mmdd));
        }
    }
    if let Ok(serial) = s.parse::<f64>() {
        if serial > 40000.0 && serial < 55000.0 {
            let days_since_epoch = serial as i64 - 25569;
            let timestamp = days_since_epoch * 86400;
            if let Some(dt) = chrono::DateTime::from_timestamp(timestamp, 0).map(|dt| dt.naive_utc()) {
                return Some(dt.format("%Y-%m-%d").to_string());
            }
        }
    }
    None
}
```

在 `mod tests` 中追加：

```rust
    #[test]
    fn test_parse_full_datetime() {
        let dt = parse_datetime("2026-04-24 17:58:59").unwrap();
        assert_eq!(dt.format("%Y-%m-%d %H:%M:%S").to_string(), "2026-04-24 17:58:59");
    }

    #[test]
    fn test_parse_no_year_uses_current_year() {
        let now = chrono::Local::now();
        let dt = parse_datetime("04-22 21:10").unwrap();
        assert_eq!(dt.year(), now.year());
        assert_eq!(dt.month(), 4);
        assert_eq!(dt.day(), 22);
    }

    #[test]
    fn test_parse_incomplete_returns_none() {
        assert_eq!(parse_datetime("04-22 21:??"), None);
    }

    #[test]
    fn test_parse_trailing_colon() {
        let now = chrono::Local::now();
        let dt = parse_datetime("04-22 21:").unwrap();
        assert_eq!(dt.year(), now.year());
    }

    #[test]
    fn test_parse_no_space() {
        assert!(parse_datetime("2026-04-2408:48:00").is_some());
    }

    #[test]
    fn test_extract_date_full() {
        assert_eq!(extract_date("2026-04-24 17:58:59").as_deref(), Some("2026-04-24"));
    }

    #[test]
    fn test_extract_date_no_year() {
        let d = extract_date("04-22 21:10").expect("应补当年");
        assert!(d.starts_with("2026-04-22"), "实际: {d}");
    }

    #[test]
    fn test_extract_date_excel_serial() {
        let d = extract_date("46134.932").expect("应解析 Excel 序列号");
        assert!(d.starts_with("2026-"), "实际: {d}");
    }
```

- [ ] **步骤 2：跑测试**

运行：`cargo test --lib parser::datetime_util`
预期：18 个单测全 PASS

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/parser/datetime_util.rs
git commit -m "feat: datetime_util 模块 Layer 2 parse_datetime + Layer 2b extract_date"
```

---

### 任务 3：parse_itinerary_from_tables 改用 extract_datetime

**文件：**
- 修改：`src-tauri/src/parser/itinerary_parser.rs:551-553`（删 `re_weekday`/`re_colon_space`）
- 修改：`src-tauri/src/parser/itinerary_parser.rs:662-671`（date_time 提取）
- 测试：`src-tauri/src/parser/itinerary_parser.rs` 内 `mod tests`

- [ ] **步骤 1：编写/更新回归测试（先失败）**

更新现有 `test_parse_itinerary_from_tables_didi_mock`（:2126）与新增的 `test_parse_itinerary_from_tables_page2_weekday_split_across_newline`（:2250），确保断言 `date_time` 不含任何周几字符（`周一`..`周日`）。在 `mod tests` 中加一个轻量断言辅助并在两个测试里调用：

```rust
fn assert_no_weekday(dt: &str) {
    for w in ["周一", "周二", "周三", "周四", "周五", "周六", "周日"] {
        assert!(!dt.contains(w), "date_time 不应含周几: '{dt}'");
    }
}
```

在 `test_parse_itinerary_from_tables_didi_mock` 末尾追加：
```rust
assert!(entries.iter().all(|e| {
    !["周一","周二","周三","周四","周五","周六","周日"].iter().any(|w| e.date_time.contains(w))
}), "所有条目时间不应含周几");
```

- [ ] **步骤 2：运行确认失败**

运行：`cargo test --features pdfplumber --lib parser::itinerary_parser::tests::test_parse_itinerary_from_tables_didi_mock -- --nocapture`
预期：当前代码（`re_weekday` 用 `周\s*[一二三四五六日天]`）在 mock 数据（周几连续）下已通过——此步骤用于确认基线。真正的失败回归由任务 1 测试覆盖。

- [ ] **步骤 3：实现替换**

删除 `:552` 的 `re_weekday` 与 `:553` 的 `re_colon_space` 两行（若不再被引用）。

将 `:662-671` 的时间提取改为：

```rust
                // 时间：直接从单元格文本按格式列表提取（周几/换行天然被忽略）
                let date_time = col_indices
                    .get(&SemanticCol::Time)
                    .and_then(|&idx| row.get(idx))
                    .and_then(|cell| crate::parser::datetime_util::extract_datetime(&cell.line_text))
                    .unwrap_or_default();
```

- [ ] **步骤 4：运行确认通过**

运行：`cargo test --features pdfplumber --lib parser::itinerary_parser`
预期：除已知失败的 `test_parse_itinerary_from_tables_tianfutong`（存量失败，与本任务无关）外全 PASS

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/parser/itinerary_parser.rs
git commit -m "fix: parse_itinerary_from_tables 改用 extract_datetime，删除 re_weekday/re_colon_space"
```

---

### 任务 4：parse_table_generic 改用 extract_datetime

**文件：**
- 修改：`src-tauri/src/parser/itinerary_parser.rs:288-295`（删 8 个 `re_datetime_*`）
- 修改：`src-tauri/src/parser/itinerary_parser.rs:407-434`（if-else 链）
- 测试：`src-tauri/src/parser/itinerary_parser.rs` 内 `mod tests`

- [ ] **步骤 1：运行现有测试确认基线**

运行：`cargo test --lib parser::itinerary_parser::tests::test_didi_split_time_with_coords parser::itinerary_parser::tests::test_merged_datehour_with_mins_continuation parser::itinerary_parser::tests::test_two_row_split_time_tight_gap`
预期：当前 3 个测试全 PASS（基线）

- [ ] **步骤 2：实现替换**

删除 `:288-295` 的 8 个 `re_datetime_*` 正则定义（确认 `re_amount` 仍被下方使用，保留）。

将 `:416-434` 的 if-else 链整体替换为：

```rust
            let time_text = clean_time_text(&raw_time_text);
            let date_time = crate::parser::datetime_util::extract_datetime(&time_text)
                .unwrap_or_else(|| time_text.trim().to_string());
```

注意：保留上方的 `let time_text = clean_time_text(&raw_time_text);` 并删除原来的 if-else 分支块。

- [ ] **步骤 3：运行确认通过**

运行：`cargo test --features pdfplumber --lib parser::itinerary_parser`
预期：3 个相关测试仍 PASS

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/parser/itinerary_parser.rs
git commit -m "refactor: parse_table_generic 8 个 re_datetime_* 正则链改用 extract_datetime"
```

---

### 任务 5：extract_toll_travel_time 改用 Layer 1+2

**文件：**
- 修改：`src-tauri/src/parser/invoice_parser.rs:1061-1079`

- [ ] **步骤 1：编写失败测试（TDD）**

在 `invoice_parser.rs` 的 `#[cfg(test)] mod tests` 中追加（若该处已有测试模块则复用）：

```rust
    #[test]
    fn test_extract_toll_travel_time_no_space() {
        // OCR 粘连格式
        let dt = extract_toll_travel_time("通行时间：2026-05-2510:06:04 入口：XX").unwrap();
        assert_eq!(dt.format("%Y-%m-%d %H:%M:%S").to_string(), "2026-05-25 10:06:04");
    }

    #[test]
    fn test_extract_toll_travel_time_date_only() {
        let dt = extract_toll_travel_time("2026-05-25 过路费").unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2026-05-25");
    }
```

- [ ] **步骤 2：运行确认失败**

运行：`cargo test --lib parser::invoice_parser::tests::test_extract_toll_travel_time_no_space -- --nocapture`
预期：`test_extract_toll_travel_time_no_space` FAIL（现有实现用 `\d{4}-\d{2}-\d{2}\s*\d{2}:\d{2}:\d{2}` 能匹配粘连格式，实际可能已通过——若已通过则断言 `date_only` 用例，现有实现也支持。若都通过，此步改为确认基线，跳过"失败"预期）。

- [ ] **步骤 3：实现替换**

将 `extract_toll_travel_time` 函数体整体替换为：

```rust
pub fn extract_toll_travel_time(remarks: &str) -> Option<chrono::NaiveDateTime> {
    crate::parser::datetime_util::extract_datetime(remarks)
        .and_then(|s| crate::parser::datetime_util::parse_datetime(&s))
}
```

- [ ] **步骤 4：运行确认通过**

运行：`cargo test --lib parser::invoice_parser`
预期：新测试 + 存量测试全 PASS

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/parser/invoice_parser.rs
git commit -m "refactor: extract_toll_travel_time 改用 datetime_util Layer 1+2"
```

---

### 任务 6：matching/batch.rs 委托 Layer 2/2b

**文件：**
- 修改：`src-tauri/src/matching/batch.rs:707-752`（parse_datetime 函数体）
- 修改：`src-tauri/src/matching/batch.rs:777-800`（extract_date 函数体）

- [ ] **步骤 1：运行现有测试确认基线**

运行：`cargo test --lib matching::batch`
预期：全 PASS（基线）

- [ ] **步骤 2：实现替换**

将 `parse_datetime`（:707）函数体替换为：

```rust
fn parse_datetime(time_str: &str) -> Option<NaiveDateTime> {
    crate::parser::datetime_util::parse_datetime(time_str)
}
```

将 `extract_date`（:777）函数体替换为：

```rust
fn extract_date(time_str: &str) -> String {
    crate::parser::datetime_util::extract_date(time_str).unwrap_or_default()
}
```

若替换后出现 `unused` 警告（FORMATS 等局部已删除），一并清理。

- [ ] **步骤 3：运行确认通过**

运行：`cargo test --lib matching::batch`
预期：全 PASS

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/matching/batch.rs
git commit -m "refactor: matching/batch parse_datetime/extract_date 委托 datetime_util"
```

---

### 任务 7：matching/engine.rs 委托 Layer 2

**文件：**
- 修改：`src-tauri/src/matching/engine.rs:12-30`

- [ ] **步骤 1：运行现有测试确认基线**

运行：`cargo test --lib matching::engine`
预期：全 PASS

- [ ] **步骤 2：实现替换**

将 `parse_payment_date`（:12-30）函数体替换为：

```rust
pub fn parse_payment_date(time_str: &str) -> Option<NaiveDate> {
    crate::parser::datetime_util::parse_datetime(time_str).map(|dt| dt.date())
}
```

- [ ] **步骤 3：运行确认通过**

运行：`cargo test --lib matching::engine`
预期：全 PASS

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/matching/engine.rs
git commit -m "refactor: matching/engine parse_payment_date 委托 datetime_util"
```

---

### 任务 8：matching/scoring.rs 委托 Layer 2

**文件：**
- 修改：`src-tauri/src/matching/scoring.rs:240-260`

- [ ] **步骤 1：运行现有测试确认基线**

运行：`cargo test --lib matching::scoring`
预期：全 PASS

- [ ] **步骤 2：实现替换**

将 `fn parse_datetime(&self, ...)`（:240-260）函数体替换为：

```rust
    fn parse_datetime(&self, time_str: &str) -> Option<NaiveDateTime> {
        crate::parser::datetime_util::parse_datetime(time_str)
    }
```

- [ ] **步骤 3：运行确认通过**

运行：`cargo test --lib matching::scoring`
预期：全 PASS

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/matching/scoring.rs
git commit -m "refactor: matching/scoring parse_datetime 委托 datetime_util"
```

---

### 任务 9：comparison_xlsx compute_time_diff 改用 Layer 2

**文件：**
- 修改：`src-tauri/src/pdf/comparison_xlsx_generator.rs:512-571`

- [ ] **步骤 1：运行现有测试确认基线**

运行：`cargo test --lib pdf::comparison_xlsx_generator`
预期：全 PASS

- [ ] **步骤 2：实现替换**

将 `compute_time_diff`（:512-571）中解析 payment_time 和 itinerary_time 的两个手工格式块替换。整体替换函数体中的解析部分（保留 `:573` 起的时长计算与返回）：

```rust
fn compute_time_diff(itinerary_time: &str, payment_time: &str) -> String {
    let pay_dt = match crate::parser::datetime_util::parse_datetime(payment_time) {
        Some(dt) => dt,
        None => return String::new(),
    };
    let pay_year = pay_dt.format("%Y").to_string();

    // 行程时间无年份时用支付年份补全（跨年行程保持一致）
    let itin_dt = crate::parser::datetime_util::parse_datetime(&format!("{}-{}", pay_year, itinerary_time))
        .or_else(|| crate::parser::datetime_util::parse_datetime(itinerary_time));
    let itin_dt = match itin_dt {
        Some(dt) => dt,
        None => return String::new(),
    };

    let duration = (pay_dt - itin_dt).num_minutes().abs();

    if duration < 1 {
        return String::new();
    }
    if duration < 60 {
        return format!("{}分钟", duration);
    }
    let hours = duration / 60;
    let mins = duration % 60;
    if mins == 0 {
        format!("{}小时", hours)
    } else {
        format!("{}小时{}分", hours, mins)
    }
}
```

注意：替换前先读取 `:573` 之后原有返回值逻辑，保持时长文案一致；若原有返回值格式不同，保留原样。

- [ ] **步骤 3：运行确认通过**

运行：`cargo test --lib pdf::comparison_xlsx_generator`
预期：全 PASS

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/pdf/comparison_xlsx_generator.rs
git commit -m "refactor: comparison_xlsx compute_time_diff 改用 datetime_util Layer 2"
```

---

### 任务 10：全量回归 + 真实 PDF 验证

**文件：** 无代码改动

- [ ] **步骤 1：行程单表格/流水线回归**

运行：
```bash
cargo test --features pdfplumber --test pdfplumber_cell_debug_test
cargo test --features pdfplumber --test pdfplumber_pipeline_test
```
预期：全 PASS

- [ ] **步骤 2：真实滴滴 PDF 第 2 页验证（确认周几不再泄漏）**

运行：`cargo test --features pdfplumber --test diag_didi_pages -- --nocapture`
预期：所有 `dt='...'` 不含周几字符（`05-11 11:48`、`06-11 19:15` 等干净时间）

- [ ] **步骤 3：lib 全量单测**

运行：`cargo test --lib`
预期：全 PASS（`test_parse_itinerary_from_tables_tianfutong` 为存量失败，如仍失败需在计划外单独处理，不作为本次回归阻断项）

- [ ] **步骤 4：Commit（如有未提交的测试改动）**

```bash
git add -A
git commit -m "test: datetime_util 全量回归 + 真实 PDF 验证"
```

---

## 自检记录

**规格覆盖度：**
- Layer 1（6 种格式）→ 任务 1 ✓
- Layer 2（无年份/`:??`）→ 任务 2 ✓
- Layer 2b（含 Excel 序列号）→ 任务 2 ✓
- parse_itinerary_from_tables → 任务 3 ✓
- parse_table_generic 8 正则链 → 任务 4 ✓
- extract_toll_travel_time → 任务 5 ✓
- matching/batch、engine、scoring → 任务 6/7/8 ✓
- comparison_xlsx compute_time_diff → 任务 9 ✓
- 回归 → 任务 10 ✓

**占位符扫描：** 无 "TODO/待定"；每步含完整代码与命令。

**类型一致性：** 各任务统一使用 `crate::parser::datetime_util::{extract_datetime, parse_datetime, extract_date}`；返回类型：`extract_datetime`→`Option<String>`，`parse_datetime`→`Option<NaiveDateTime>`，`extract_date`→`Option<String>`，跨任务一致。
