# 多趟出差自动分趟 & 分别导出 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 一次导入多趟出差发票后，按机票/火车票的链式行程自动分组为多笔出差单，用户可手动调整分组（含选定出发城市重新匹配）后每趟分别导出报销文件。

**架构：**
- 后端新增纯函数 `segment_trips`（`matching/segment.rs`）：筛选票据 → 链式贪心成组（自动模式 origin 由首张票推断；指定 origin 模式返程放宽为"到达 origin"）→ 非票据发票按日期落入趟窗口。输出趟分组 + 待调整列表。
- 前端 match store 增加 `trips`/`unassigned`/`segmentOrigin` 状态与 `resegment`/`moveToTrip`/`createTripFromTicket` 操作；导出页改为逐趟卡片 + 待调整区 + 出发城市重匹配工具栏；导出命令零改动（复用现有按 `match_results` 子集生成）。

**技术栈：** Rust (Tauri 2 command + serde)、Vue 3 + Pinia + TypeScript、Vitest（前端单测）、cargo test（后端单测）。

**规格：** `docs/superpowers/specs/2026-07-31-multi-trip-segmentation-design.md`

---

## 文件结构

- 创建：`src-tauri/src/matching/segment.rs` — 分趟算法（纯函数 + 内联单元测试）
- 修改：`src-tauri/src/matching/mod.rs` — 注册 `segment` 模块
- 修改：`src-tauri/src/lib.rs` — 新增 `segment_trips` Tauri 命令并注册
- 修改：`src/types/match.ts` — 新增 `Trip` 接口
- 修改：`src/stores/match.ts` — 分趟状态与操作；`renderReimbursementHtml` 支持传入 matches 子集；移除已无用的 `exportForm`
- 创建：`src/__tests__/match-store-segment.test.ts` — store 分趟逻辑 Vitest 单测
- 创建：`src/components/TripCard.vue` — 单趟出差卡片（表单 + 发票明细 + 移动控件 + 导出按钮）
- 修改：`src/views/ExportView.vue` — 分趟区 + 待调整区 + 出发城市重匹配工具栏 + 逐趟预览导出

---

## 任务 1：后端分趟算法模块（TDD）

**文件：**
- 创建：`src-tauri/src/matching/segment.rs`
- 修改：`src-tauri/src/matching/mod.rs`

- [ ] **步骤 1：编写失败的单元测试**

在 `src-tauri/src/matching/segment.rs` 写入以下内容（测试先行，`segment_trips` 尚未实现，编译失败即"测试失败"）：

```rust
use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use chrono::NaiveDate;

use crate::models::invoice::{Invoice, InvoiceCategory};
use crate::models::match_result::MatchResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripGroup {
    pub id: String,
    pub destination: String,
    pub travel_start: String,
    pub travel_end: String,
    pub ticket_ids: Vec<String>,
    pub invoice_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentResult {
    pub trips: Vec<TripGroup>,
    pub unassigned_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct Ticket {
    invoice_id: String,
    departure: String,
    arrival: String,
    travel_date: NaiveDate,
}

/// 主入口：origin 为 None 时自动分趟，Some 时以指定出发城市全量重分
pub fn segment_trips(match_results: &[MatchResult], origin: Option<&str>) -> SegmentResult {
    todo!("实现分趟算法")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{HotelDetail, InvoiceSource};
    use crate::models::match_result::MatchType;

    fn inv(id: &str, cat: InvoiceCategory, date: &str) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: String::new(),
            amount: 100.0,
            seller_name: String::new(),
            item_name: String::new(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            travel_date: None,
            category: cat,
            source: InvoiceSource::Manual,
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: None,
        }
    }

    fn ticket(id: &str, dep: &str, arr: &str, travel_date: &str) -> Invoice {
        let mut i = inv(id, InvoiceCategory::Train, travel_date);
        i.travel_date = NaiveDate::parse_from_str(travel_date, "%Y-%m-%d").ok();
        i.departure_city = Some(dep.to_string());
        i.arrival_city = Some(arr.to_string());
        i
    }

    fn flight(id: &str, dep: &str, arr: &str, travel_date: &str) -> Invoice {
        let mut i = inv(id, InvoiceCategory::Flight, travel_date);
        i.travel_date = NaiveDate::parse_from_str(travel_date, "%Y-%m-%d").ok();
        i.departure_city = Some(dep.to_string());
        i.arrival_city = Some(arr.to_string());
        i
    }

    fn mr(invoice: Invoice) -> MatchResult {
        MatchResult {
            invoice_id: invoice.id.clone(),
            invoice,
            payment_ids: vec![],
            payments: vec![],
            match_type: MatchType::Unmatched,
            confidence: 1.0,
            amount_diff: 0.0,
            itinerary_payment_pairs: vec![],
            shared_payment_ids: vec![],
            shared_from_invoice_id: None,
        }
    }

    #[test]
    fn test_auto_simple_round_trip() {
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "上海", "长沙", "2026-05-22")),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 1);
        let t = &seg.trips[0];
        assert_eq!(t.destination, "上海");
        assert_eq!(t.travel_start, "2026-05-20");
        assert_eq!(t.travel_end, "2026-05-22");
        assert_eq!(t.ticket_ids, vec!["t1".to_string(), "t2".to_string()]);
        assert!(seg.unassigned_ids.is_empty());
    }

    #[test]
    fn test_auto_chain_round_trip() {
        let results = vec![
            mr(ticket("t1", "长沙", "武汉", "2026-05-20")),
            mr(ticket("t2", "武汉", "北京", "2026-05-21")),
            mr(flight("t3", "北京", "长沙", "2026-05-23")),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 1);
        let t = &seg.trips[0];
        assert_eq!(t.destination, "北京");
        assert_eq!(t.travel_start, "2026-05-20");
        assert_eq!(t.travel_end, "2026-05-23");
        assert_eq!(t.ticket_ids, vec!["t1".to_string(), "t2".to_string(), "t3".to_string()]);
        assert!(seg.unassigned_ids.is_empty());
    }

    #[test]
    fn test_auto_multiple_trips() {
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "上海", "长沙", "2026-05-22")),
            mr(ticket("t3", "长沙", "成都", "2026-06-10")),
            mr(ticket("t4", "成都", "长沙", "2026-06-12")),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 2);
        assert_eq!(seg.trips[0].destination, "上海");
        assert_eq!(seg.trips[1].destination, "成都");
        assert!(seg.unassigned_ids.is_empty());
    }

    #[test]
    fn test_auto_one_way_goes_unassigned() {
        let results = vec![mr(ticket("t1", "长沙", "上海", "2026-05-20"))];
        let seg = segment_trips(&results, None);
        assert!(seg.trips.is_empty());
        assert_eq!(seg.unassigned_ids, vec!["t1".to_string()]);
    }

    #[test]
    fn test_auto_broken_chain_unassigned() {
        let results = vec![
            mr(ticket("t1", "长沙", "武汉", "2026-05-20")),
            mr(ticket("t2", "北京", "长沙", "2026-05-23")),
        ];
        let seg = segment_trips(&results, None);
        assert!(seg.trips.is_empty());
        assert_eq!(seg.unassigned_ids, vec!["t1".to_string(), "t2".to_string()]);
    }

    #[test]
    fn test_auto_window_assignment() {
        let mut hotel = inv("h1", InvoiceCategory::Hotel, "2026-05-21");
        hotel.hotel_detail = Some(HotelDetail {
            check_in: NaiveDate::parse_from_str("2026-05-20", "%Y-%m-%d").unwrap(),
            check_out: None,
            nights: 1,
            nightly_rate: 300.0,
        });
        let mut meal = inv("m1", InvoiceCategory::Meal, "2026-05-21");
        meal.date = NaiveDate::parse_from_str("2026-05-21", "%Y-%m-%d").unwrap();
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "上海", "长沙", "2026-05-22")),
            mr(hotel),
            mr(meal),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 1);
        let mut ids = seg.trips[0].invoice_ids.clone();
        ids.sort();
        assert_eq!(ids, vec!["h1".to_string(), "m1".to_string(), "t1".to_string(), "t2".to_string()]);
        assert!(seg.unassigned_ids.is_empty());
    }

    #[test]
    fn test_auto_meal_outside_window_unassigned() {
        let mut meal = inv("m1", InvoiceCategory::Meal, "2026-06-10");
        meal.date = NaiveDate::parse_from_str("2026-06-10", "%Y-%m-%d").unwrap();
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "上海", "长沙", "2026-05-22")),
            mr(meal),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 1);
        assert_eq!(seg.unassigned_ids, vec!["m1".to_string()]);
    }

    #[test]
    fn test_origin_pairs_broken_chain() {
        let results = vec![
            mr(ticket("t1", "长沙", "武汉", "2026-05-20")),
            mr(ticket("t2", "北京", "长沙", "2026-05-23")),
        ];
        let seg = segment_trips(&results, Some("长沙"));
        assert_eq!(seg.trips.len(), 1);
        let t = &seg.trips[0];
        assert_eq!(t.destination, "北京");
        assert_eq!(t.travel_start, "2026-05-20");
        assert_eq!(t.travel_end, "2026-05-23");
        assert_eq!(t.ticket_ids, vec!["t1".to_string(), "t2".to_string()]);
        assert!(seg.unassigned_ids.is_empty());
    }

    #[test]
    fn test_origin_chain_round_trip() {
        let results = vec![
            mr(ticket("t1", "长沙", "武汉", "2026-05-20")),
            mr(ticket("t2", "武汉", "北京", "2026-05-21")),
            mr(flight("t3", "北京", "长沙", "2026-05-23")),
        ];
        let seg = segment_trips(&results, Some("长沙"));
        assert_eq!(seg.trips.len(), 1);
        assert_eq!(seg.trips[0].destination, "北京");
        assert_eq!(seg.trips[0].ticket_ids, vec!["t1".to_string(), "t2".to_string(), "t3".to_string()]);
    }

    #[test]
    fn test_origin_first_ticket_not_from_origin() {
        let results = vec![
            mr(ticket("t1", "北京", "长沙", "2026-05-20")),
            mr(ticket("t2", "长沙", "上海", "2026-05-22")),
        ];
        let seg = segment_trips(&results, Some("长沙"));
        assert!(seg.trips.is_empty());
        assert_eq!(seg.unassigned_ids, vec!["t1".to_string(), "t2".to_string()]);
    }

    #[test]
    fn test_origin_new_outbound_closes_incomplete() {
        let results = vec![
            mr(ticket("t1", "长沙", "武汉", "2026-05-20")),
            mr(ticket("t2", "长沙", "北京", "2026-05-25")),
            mr(ticket("t3", "北京", "长沙", "2026-05-27")),
        ];
        let seg = segment_trips(&results, Some("长沙"));
        assert_eq!(seg.trips.len(), 1);
        assert_eq!(seg.trips[0].ticket_ids, vec!["t2".to_string(), "t3".to_string()]);
        assert_eq!(seg.unassigned_ids, vec!["t1".to_string()]);
    }

    #[test]
    fn test_origin_foreign_ticket_in_window_unassigned() {
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "苏州", "上海", "2026-05-21")),
            mr(ticket("t3", "上海", "长沙", "2026-05-22")),
        ];
        let seg = segment_trips(&results, Some("长沙"));
        assert_eq!(seg.trips.len(), 1);
        assert_eq!(seg.trips[0].ticket_ids, vec!["t1".to_string(), "t3".to_string()]);
        assert_eq!(seg.unassigned_ids, vec!["t2".to_string()]);
    }

    #[test]
    fn test_auto_city_transport_uses_itinerary_date() {
        let mut ct = inv("c1", InvoiceCategory::CityTransport, "2026-05-21");
        ct.itineraries = vec![crate::models::invoice::Itinerary {
            date_time: "2026-05-20 08:30".to_string(),
            provider: "滴滴".to_string(),
            pickup: "长沙站".to_string(),
            dropoff: "国贸".to_string(),
            amount: 35.0,
            incomplete_fields: vec![],
        }];
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "上海", "长沙", "2026-05-22")),
            mr(ct),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips[0].invoice_ids.len(), 3);
        assert!(seg.trips[0].invoice_ids.iter().any(|id| id == "c1"));
        assert!(seg.unassigned_ids.is_empty());
    }
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：`cargo test -p invoice-reimbursement segment 2>&1 | Select-String -Pattern "error|warning: unused|test result"`
预期：编译失败，报错 `not found in this scope` / `cannot find function segment_trips`（`todo!()` 也会使测试 panic 失败）。

- [ ] **步骤 3：实现分趟算法**

用以下完整实现替换 `segment_trips` 主体（保留上方已写入的 struct/Ticket/主入口签名，替换 `todo!()` 一行，并补全各辅助函数）：

```rust
/// 从已匹配发票中筛选票据（Train/Flight 且城市+出行日期齐全），按出行日期排序
fn collect_tickets(match_results: &[MatchResult]) -> Vec<Ticket> {
    let mut tickets: Vec<Ticket> = match_results
        .iter()
        .filter_map(|m| {
            let inv = &m.invoice;
            if inv.category != InvoiceCategory::Train && inv.category != InvoiceCategory::Flight {
                return None;
            }
            match (&inv.departure_city, &inv.arrival_city, inv.travel_date) {
                (Some(dep), Some(arr), Some(date)) => Some(Ticket {
                    invoice_id: inv.id.clone(),
                    departure: dep.clone(),
                    arrival: arr.clone(),
                    travel_date: date,
                }),
                _ => None,
            }
        })
        .collect();
    tickets.sort_by_key(|t| t.travel_date);
    tickets
}

fn fmt(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// 无 origin：首张票推断 origin，返程需「出发==frontier 且 到达==origin」（链式严格衔接）。
/// 链断时链上票据进待调整，断点票据作为下一趟第 1 程。
fn segment_auto(tickets: &[Ticket]) -> (Vec<TripGroup>, Vec<String>) {
    let mut trips = Vec::new();
    let mut unassigned = Vec::new();
    let mut i = 0;
    while i < tickets.len() {
        let t0 = &tickets[i];
        let origin = t0.departure.clone();
        let mut frontier = t0.arrival.clone();
        let mut chain: Vec<String> = vec![t0.invoice_id.clone()];
        let mut j = i + 1;
        let mut ret: Option<&Ticket> = None;
        while j < tickets.len() {
            let t = &tickets[j];
            if t.departure == frontier && t.arrival == origin {
                ret = Some(t);
                break;
            }
            if t.departure == frontier {
                chain.push(t.invoice_id.clone());
                frontier = t.arrival.clone();
                j += 1;
                continue;
            }
            break;
        }
        match ret {
            Some(r) => {
                chain.push(r.invoice_id.clone());
                trips.push(TripGroup {
                    id: format!("trip-{}", trips.len() + 1),
                    destination: r.departure.clone(),
                    travel_start: fmt(t0.travel_date),
                    travel_end: fmt(r.travel_date),
                    ticket_ids: chain.clone(),
                    invoice_ids: chain,
                });
                i = j + 1;
            }
            None => {
                unassigned.extend(chain);
                i = j;
            }
        }
    }
    (trips, unassigned)
}

/// 带 origin：去程=从 origin 出发，返程=到达 origin（出发城市不限），链式续程。
fn segment_with_origin(tickets: &[Ticket], origin: &str) -> (Vec<TripGroup>, Vec<String>) {
    let mut trips = Vec::new();
    let mut unassigned = Vec::new();
    let mut open: Option<(Vec<String>, String, NaiveDate)> = None; // (chain, frontier, start_date)
    for t in tickets {
        match &mut open {
            None => {
                if t.departure == origin {
                    open = Some((vec![t.invoice_id.clone()], t.arrival.clone(), t.travel_date));
                } else {
                    unassigned.push(t.invoice_id.clone());
                }
            }
            Some((chain, frontier, start_date)) => {
                if t.arrival == origin {
                    chain.push(t.invoice_id.clone());
                    trips.push(TripGroup {
                        id: format!("trip-{}", trips.len() + 1),
                        destination: t.departure.clone(),
                        travel_start: fmt(*start_date),
                        travel_end: fmt(t.travel_date),
                        ticket_ids: chain.clone(),
                        invoice_ids: chain.clone(),
                    });
                    open = None;
                } else if t.departure == *frontier {
                    chain.push(t.invoice_id.clone());
                    frontier.clone_from(&t.arrival);
                } else if t.departure == origin {
                    unassigned.extend(chain.drain(..));
                    frontier.clone_from(&t.arrival);
                    chain.push(t.invoice_id.clone());
                    *start_date = t.travel_date;
                } else {
                    unassigned.push(t.invoice_id.clone());
                }
            }
        }
    }
    if let Some((chain, _, _)) = open {
        unassigned.extend(chain);
    }
    (trips, unassigned)
}

/// 非票据发票的归口日期
fn effective_date(inv: &Invoice) -> Option<NaiveDate> {
    match inv.category {
        InvoiceCategory::Hotel => inv.hotel_detail.as_ref().and_then(|h| h.check_in).or(Some(inv.date)),
        InvoiceCategory::CityTransport => inv
            .itineraries
            .first()
            .and_then(|it| it.date_time.split_whitespace().next())
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .or(Some(inv.date)),
        InvoiceCategory::Toll => inv.toll_travel_time.map(|t| t.date()).or(Some(inv.date)),
        _ => Some(inv.date),
    }
}

/// 非票据发票按日期落入趟窗口 [start, end]；跨窗口取最早开始；窗口外进待调整。
/// 票据（含完整票据与缺字段票据的补位处理）只随链归属，绝不按窗口归入。
fn assign_by_date(
    match_results: &[MatchResult],
    tickets: &[Ticket],
    trips: &mut Vec<TripGroup>,
    unassigned: &mut Vec<String>,
) {
    let ticket_ids: HashSet<&str> = tickets.iter().map(|t| t.invoice_id.as_str()).collect();
    let windows: Vec<(NaiveDate, NaiveDate)> = trips
        .iter()
        .map(|t| {
            (
                NaiveDate::parse_from_str(&t.travel_start, "%Y-%m-%d").unwrap_or_default(),
                NaiveDate::parse_from_str(&t.travel_end, "%Y-%m-%d").unwrap_or_default(),
            )
        })
        .collect();

    let mut assigned: HashMap<&str, usize> = HashMap::new();
    let mut unassigned_set: HashSet<String> = unassigned.iter().cloned().collect();

    for m in match_results {
        let inv = &m.invoice;
        if ticket_ids.contains(inv.id.as_str()) {
            continue;
        }
        let Some(date) = effective_date(inv) else { continue };
        let mut best: Option<usize> = None;
        for (i, (s, e)) in windows.iter().enumerate() {
            if date >= *s && date <= *e {
                match best {
                    None => best = Some(i),
                    Some(b) if windows[i].0 < windows[b].0 => best = Some(i),
                    _ => {}
                }
            }
        }
        match best {
            Some(i) => {
                assigned.insert(inv.id.as_str(), i);
            }
            None => {
                unassigned_set.insert(inv.id.clone());
            }
        }
    }

    for (id, ti) in assigned {
        trips[ti].invoice_ids.push(id.to_string());
    }
    for trip in trips.iter_mut() {
        trip.invoice_ids.sort();
    }
    *unassigned = unassigned_set.into_iter().collect();
    unassigned.sort();
}

/// 主入口：origin 为 None 时自动分趟，Some 时以指定出发城市全量重分
pub fn segment_trips(match_results: &[MatchResult], origin: Option<&str>) -> SegmentResult {
    let tickets = collect_tickets(match_results);
    let (mut trips, mut unassigned) = match origin {
        Some(o) => segment_with_origin(&tickets, o),
        None => segment_auto(&tickets),
    };
    assign_by_date(match_results, &tickets, &mut trips, &mut unassigned);
    SegmentResult { trips, unassigned_ids: unassigned }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -p invoice-reimbursement segment 2>&1 | Select-String -Pattern "test result|FAILED"`
预期：`test result: ok`，12 个测试全部 PASS。

- [ ] **步骤 5：注册模块**

修改 `src-tauri/src/matching/mod.rs`，在 `pub mod benchmarks;` 后新增一行：

```rust
pub mod segment;
```

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/matching/segment.rs src-tauri/src/matching/mod.rs
git commit -m "feat(segment): 多趟出差链式分组算法（自动/指定origin）+ 单测"
```

---

## 任务 2：后端 Tauri 命令

**文件：**
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：新增命令**

在 `src-tauri/src/lib.rs` 中 `generate_comparison_xlsx` 命令（约 596-603 行）之后、`run()` 之前新增：

```rust
// 多趟出差自动分趟命令：origin 为 None 自动分趟，Some 时以指定出发城市全量重分
#[tauri::command]
fn segment_trips(
    match_results: Vec<MatchResult>,
    origin: Option<String>,
) -> Result<matching::segment::SegmentResult, String> {
    Ok(matching::segment::segment_trips(&match_results, origin.as_deref()))
}
```

- [ ] **步骤 2：注册命令**

在 `run()` 的 `invoke_handler` 列表（约 613-640 行）末尾 `open_file_with_system,` 后新增一行：

```rust
            segment_trips,
```

- [ ] **步骤 3：编译验证**

运行：`cargo check -p invoice-reimbursement 2>&1 | Select-String -Pattern "error|warning: unused|Finished"`
预期：无 error，输出 `Finished`（首次编译 pdfplumber git 依赖可能需要数分钟）。

- [ ] **步骤 4：运行全部后端测试**

运行：`cargo test -p invoice-reimbursement 2>&1 | Select-String -Pattern "test result"`
预期：多个 `test result: ok`（含 `segment` 模块测试）。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(segment): 暴露 segment_trips Tauri 命令"
```

---

## 任务 3：前端类型与 store 分趟逻辑（TDD）

**文件：**
- 修改：`src/types/match.ts`
- 修改：`src/stores/match.ts`
- 创建：`src/__tests__/match-store-segment.test.ts`

- [ ] **步骤 1：新增 Trip 类型**

在 `src/types/match.ts` 末尾新增：

```ts
/// 一趟出差分组：destination/travelStart/travelEnd 预填自票据，用户可手动修改
export interface Trip {
  id: string
  destination: string
  travelStart: string
  travelEnd: string
  hotelLevel: string
  ticketIds: string[]
  matches: MatchResult[]
}
```

- [ ] **步骤 2：编写失败的 store 单测**

创建 `src/__tests__/match-store-segment.test.ts`：

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))

import { createPinia, setActivePinia } from 'pinia'
import { useMatchStore } from '../stores/match'
import type { Invoice, MatchResult } from '../types'

function makeInvoice(id: string, opts: Partial<Invoice> = {}): Invoice {
  return {
    id,
    invoice_number: '',
    amount: 100,
    seller_name: '',
    item_name: '',
    date: '2026-05-20',
    category: 'Train',
    source: { type: 'Manual' },
    itineraries: [],
    ...opts,
  }
}

function makeMatch(id: string, opts: Partial<Invoice> = {}): MatchResult {
  const invoice = makeInvoice(id, opts)
  return {
    invoice_id: id,
    invoice,
    payment_ids: [],
    payments: [],
    match_type: 'OneToOne',
    confidence: 1,
    amount_diff: 0,
    itinerary_payment_pairs: [],
  }
}

describe('matchStore 分趟逻辑', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invokeMock.mockReset()
  })

  it('resegment 映射 trips 与 unassigned', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({
      trips: [{
        id: 'trip-1', destination: '上海', travel_start: '2026-05-20', travel_end: '2026-05-22',
        ticket_ids: ['t1', 't2'], invoice_ids: ['t1', 't2'],
      }],
      unassigned_ids: ['m1'],
    })
    const matches = [
      makeMatch('t1', { departure_city: '长沙', arrival_city: '上海', travel_date: '2026-05-20' }),
      makeMatch('t2', { departure_city: '上海', arrival_city: '长沙', travel_date: '2026-05-22' }),
      makeMatch('m1', { category: 'Meal' }),
    ]
    await store.resegment(matches, '')
    expect(store.trips).toHaveLength(1)
    expect(store.trips[0].destination).toBe('上海')
    expect(store.trips[0].travelStart).toBe('2026-05-20')
    expect(store.trips[0].travelEnd).toBe('2026-05-22')
    expect(store.trips[0].ticketIds).toEqual(['t1', 't2'])
    expect(store.trips[0].matches.map(m => m.invoice_id)).toEqual(['t1', 't2'])
    expect(store.unassigned.map(m => m.invoice_id)).toEqual(['m1'])
    expect(invokeMock).toHaveBeenCalledWith('segment_trips', {
      matchResults: matches,
      origin: null,
    })
  })

  it('resegment 携带 origin', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({ trips: [], unassigned_ids: ['t1'] })
    await store.resegment([makeMatch('t1')], '长沙')
    expect(invokeMock).toHaveBeenCalledWith('segment_trips', {
      matchResults: expect.anything(),
      origin: '长沙',
    })
  })

  it('无票据时兜底为单趟', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({ trips: [], unassigned_ids: ['m1', 'm2'] })
    const matches = [
      makeMatch('m1', { category: 'Meal' }),
      makeMatch('m2', { category: 'Hotel' }),
    ]
    await store.resegment(matches, '')
    expect(store.trips).toHaveLength(1)
    expect(store.trips[0].matches).toHaveLength(2)
    expect(store.unassigned).toHaveLength(0)
  })

  it('moveToTrip 移到另一趟/待调整', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({
      trips: [
        { id: 'trip-1', destination: '上海', travel_start: '2026-05-20', travel_end: '2026-05-22', ticket_ids: ['t1'], invoice_ids: ['t1'] },
        { id: 'trip-2', destination: '成都', travel_start: '2026-06-01', travel_end: '2026-06-03', ticket_ids: ['t2'], invoice_ids: ['t2'] },
      ],
      unassigned_ids: [],
    })
    const matches = [makeMatch('t1'), makeMatch('t2')]
    await store.resegment(matches, '')

    store.moveToTrip('t1', 'trip-2')
    expect(store.trips[0].matches).toHaveLength(0)
    expect(store.trips[1].matches.map(m => m.invoice_id)).toEqual(['t2', 't1'])

    store.moveToTrip('t1', null)
    expect(store.unassigned.map(m => m.invoice_id)).toEqual(['t1'])

    store.moveToTrip('t1', 'trip-1')
    expect(store.trips[0].matches.map(m => m.invoice_id)).toEqual(['t1'])
    expect(store.unassigned).toHaveLength(0)
  })

  it('createTripFromTicket 从待调整票据新建出差', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({ trips: [], unassigned_ids: ['t1'] })
    const ticket = makeMatch('t1', {
      departure_city: '长沙', arrival_city: '武汉', travel_date: '2026-05-20',
    })
    await store.resegment([ticket], '')
    store.createTripFromTicket(ticket)
    expect(store.trips).toHaveLength(1)
    expect(store.trips[0].destination).toBe('武汉')
    expect(store.trips[0].travelStart).toBe('2026-05-20')
    expect(store.trips[0].travelEnd).toBe('2026-05-20')
    expect(store.trips[0].ticketIds).toEqual(['t1'])
    expect(store.unassigned).toHaveLength(0)
  })
})
```

- [ ] **步骤 3：运行测试确认失败**

运行：`npx vitest run src/__tests__/match-store-segment.test.ts`
预期：FAIL（`store.resegment is not a function`）。

- [ ] **步骤 4：实现 store 分趟逻辑**

修改 `src/stores/match.ts`：

在文件顶部 import 处加入 `Trip` 类型（改第 3 行），并移除不再使用的 `reactive`（第 2 行）：

```ts
import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { MatchResult, Invoice, PaymentRecord, InvoiceCategory, ItineraryPaymentPair, Trip } from '../types'
import { invoke } from '@tauri-apps/api/core'
```

新增 store 内部接口（放在 `exportForm` 声明之前）：

```ts
// 后端 segment_trips 返回的趟分组（snake_case）
interface TripGroupDto {
  id: string
  destination: string
  travel_start: string
  travel_end: string
  ticket_ids: string[]
  invoice_ids: string[]
}
```

删除 `exportForm` 相关代码块（第 13-19 行的 `exportForm = reactive({...})`，以及 `clearMatches` 中第 149-152 行的 4 行重置），并替换为分趟状态：

```ts
  const trips = ref<Trip[]>([])
  const unassigned = ref<MatchResult[]>([])
  const segmentOrigin = ref('')

  function isTicket(inv: Invoice) {
    return inv.category === 'Train' || inv.category === 'Flight'
  }

  async function resegment(matches: MatchResult[], origin: string) {
    const result = await invoke<{ trips: TripGroupDto[]; unassigned_ids: string[] }>('segment_trips', {
      matchResults: matches,
      origin: origin || null,
    })
    trips.value = result.trips.map(t => ({
      id: t.id,
      destination: t.destination,
      travelStart: t.travel_start,
      travelEnd: t.travel_end,
      hotelLevel: '其他人员',
      ticketIds: t.ticket_ids,
      matches: t.invoice_ids
        .map(id => matches.find(m => m.invoice_id === id))
        .filter((m): m is MatchResult => !!m),
    }))
    unassigned.value = result.unassigned_ids
      .map(id => matches.find(m => m.invoice_id === id))
      .filter((m): m is MatchResult => !!m)
    // 兜底：无任何票据时全部作为单趟展示（保持原有单张导出可用）
    if (trips.value.length === 0 && !matches.some(m => isTicket(m.invoice))) {
      trips.value = [{
        id: 'trip-1',
        destination: '',
        travelStart: '',
        travelEnd: '',
        hotelLevel: '其他人员',
        ticketIds: [],
        matches,
      }]
      unassigned.value = []
    }
  }

  function moveToTrip(invoiceId: string, targetTripId: string | null) {
    let match: MatchResult | undefined
    for (const trip of trips.value) {
      const idx = trip.matches.findIndex(m => m.invoice_id === invoiceId)
      if (idx >= 0) {
        match = trip.matches.splice(idx, 1)[0]
        break
      }
    }
    if (!match) {
      const idx = unassigned.value.findIndex(m => m.invoice_id === invoiceId)
      if (idx >= 0) match = unassigned.value.splice(idx, 1)[0]
    }
    if (!match) return
    if (targetTripId === null) {
      unassigned.value.push(match)
      return
    }
    const target = trips.value.find(t => t.id === targetTripId)
    if (target) target.matches.push(match)
  }

  function createTripFromTicket(match: MatchResult) {
    trips.value.push({
      id: `trip-${Date.now()}`,
      destination: match.invoice.arrival_city || '',
      travelStart: match.invoice.travel_date || '',
      travelEnd: match.invoice.travel_date || '',
      hotelLevel: '其他人员',
      ticketIds: [match.invoice_id],
      matches: [match],
    })
    unassigned.value = unassigned.value.filter(m => m.invoice_id !== match.invoice_id)
  }
```

修改 `autoMatch`（第 21-37 行），在设置匹配结果后触发分趟：

```ts
  async function autoMatch(invoices: Invoice[], payments: PaymentRecord[], tolerance = 1.0) {
    loading.value = true
    try {
      const result = await invoke<{ matched: MatchResult[]; unmatched_invoices: Invoice[]; unmatched_payments: PaymentRecord[] }>(
        'auto_match', { invoices, payments, tolerance }
      )
      matches.value = result.matched
      unmatchedInvoices.value = result.unmatched_invoices
      unmatchedPayments.value = result.unmatched_payments
      await resegment(matches.value, segmentOrigin.value)
    } catch (e) {
      console.error('自动匹配失败:', e)
      throw e
    } finally {
      loading.value = false
    }
  }
```

修改 `clearMatches`（第 143-153 行），重置分趟状态：

```ts
  function clearMatches() {
    matches.value = []
    unmatchedInvoices.value = []
    unmatchedPayments.value = []
    reimbursementHtml.value = null
    trips.value = []
    unassigned.value = []
    segmentOrigin.value = ''
  }
```

修改 `renderReimbursementHtml`（第 94-118 行），支持传入 matches 子集（用于逐趟预览）：

```ts
  async function renderReimbursementHtml(
    formInfo: {
      name: string
      department: string
      destination: string
      travelStart: string
      travelEnd: string
      companions: number
      hotelLevel: string
    },
    matchesOverride?: MatchResult[],
  ) {
    const results = matchesOverride ?? matches.value
    if (results.length === 0) {
      reimbursementHtml.value = null
      return
    }
    const html = await invoke<string>('render_reimbursement_html', {
      matchResults: results,
      name: formInfo.name,
      department: formInfo.department,
      destination: formInfo.destination,
      travelStart: formInfo.travelStart,
      travelEnd: formInfo.travelEnd,
      companions: formInfo.companions,
      hotelLevel: formInfo.hotelLevel,
    })
    reimbursementHtml.value = html
  }
```

修改 return（第 155-159 行）：

```ts
  return {
    matches, unmatchedInvoices, unmatchedPayments, loading, reimbursementHtml,
    trips, unassigned, segmentOrigin,
    autoMatch, unmatchInvoice, manualMatch, removePayment, updateInvoiceCategory,
    renderReimbursementHtml, saveReimbursementHtml, clearMatches,
    resegment, moveToTrip, createTripFromTicket,
  }
```

- [ ] **步骤 5：运行测试验证通过**

运行：`npx vitest run src/__tests__/match-store-segment.test.ts`
预期：PASS，5 个用例全过。

- [ ] **步骤 6：确认无遗留 exportForm 引用**

运行：`rg "exportForm" src`
预期：仅在 `src/components/ExportButton.vue` 出现（其内部 `exportFormHtml`/`exportFormXlsx` 为函数名，与本 store 字段无关）。`src/views/ExportView.vue` 的引用将在任务 5 一并移除。

- [ ] **步骤 7：Commit**

```bash
git add src/types/match.ts src/stores/match.ts src/__tests__/match-store-segment.test.ts
git commit -m "feat(ui): match store 分趟状态与操作（resegment/moveToTrip/createTripFromTicket）"
```

---

## 任务 4：TripCard 组件

**文件：**
- 创建：`src/components/TripCard.vue`

- [ ] **步骤 1：创建组件**

创建 `src/components/TripCard.vue`：

```vue
<template>
  <div class="bg-white rounded-lg border p-5 shadow-sm space-y-4">
    <div class="flex items-center justify-between flex-wrap gap-2">
      <div class="flex items-center gap-3 flex-wrap">
        <span class="px-2 py-1 rounded bg-blue-100 text-blue-700 text-xs font-medium">出差 {{ index }}</span>
        <span class="font-medium">目的地：{{ trip.destination || '未设置' }}</span>
        <span class="text-sm text-gray-600">{{ trip.travelStart }} 至 {{ trip.travelEnd }}</span>
      </div>
      <div class="text-sm text-gray-500">
        票据 {{ trip.ticketIds.length }} · 发票 {{ trip.matches.length }} · 合计
        <span class="font-medium text-gray-800">¥{{ tripTotal.toFixed(2) }}</span>
      </div>
    </div>

    <ReimbursementForm :model-value="formModel" @update="handleFormUpdate" />

    <div class="border rounded">
      <button @click="showInvoices = !showInvoices"
              class="w-full flex items-center justify-between px-3 py-2 text-sm text-gray-600 hover:bg-gray-50">
        <span>发票明细（{{ trip.matches.length }}）</span>
        <span>{{ showInvoices ? '▾' : '▸' }}</span>
      </button>
      <div v-if="showInvoices" class="divide-y divide-gray-100">
        <div v-for="m in trip.matches" :key="m.invoice_id"
             class="flex items-center gap-2 px-3 py-2 text-sm">
          <span class="w-20 shrink-0 text-xs font-medium" :class="getCategoryBadgeClass(m.invoice.category)">
            {{ CATEGORY_LABELS[m.invoice.category] }}
          </span>
          <span class="text-gray-500 truncate flex-1">{{ m.invoice.seller_name || m.invoice.invoice_number || m.invoice.id }}</span>
          <span class="text-gray-500 shrink-0">{{ m.invoice.travel_date || m.invoice.date }}</span>
          <span class="text-gray-800 shrink-0">¥{{ m.invoice.amount.toFixed(2) }}</span>
          <select :value="trip.id" @change="handleMoveInvoice(m.invoice_id, ($event.target as HTMLSelectElement).value)"
                  class="text-xs border rounded px-1 py-0.5 shrink-0">
            <option v-for="t in otherTrips" :key="t.id" :value="t.id">移到出差 {{ t.destination || t.id }}</option>
            <option value="">移到待调整</option>
          </select>
        </div>
      </div>
    </div>

    <div class="flex gap-3">
      <button @click="$emit('preview')"
              class="px-3 py-2 rounded bg-gray-500 text-white text-sm hover:bg-gray-600 shrink-0">
        预览
      </button>
      <ExportButton
        :match-results="trip.matches"
        :unmatched-invoice-ids="[]"
        :unmatched-payment-ids="[]"
        :form-info="formInfo"
        class="flex-1"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import ReimbursementForm from './ReimbursementForm.vue'
import ExportButton from './ExportButton.vue'
import type { Trip, MatchResult } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass } from '../utils/category'

const props = defineProps<{
  trip: Trip
  index: number
  otherTrips: Trip[]
}>()

const emit = defineEmits<{
  (e: 'move', invoiceId: string, targetTripId: string | null): void
  (e: 'form-update', tripId: string, form: { destination: string; travelStart: string; travelEnd: string; hotelLevel: string }): void
  (e: 'preview'): void
}>()

const showInvoices = ref(false)

const tripTotal = computed(() => props.trip.matches.reduce((s, m) => s + m.invoice.amount, 0))

const formModel = computed(() => ({
  destination: props.trip.destination,
  travelStart: props.trip.travelStart,
  travelEnd: props.trip.travelEnd,
  hotelLevel: props.trip.hotelLevel,
}))

const formInfo = computed(() => ({
  name: '',
  department: '',
  destination: props.trip.destination,
  travelStart: props.trip.travelStart,
  travelEnd: props.trip.travelEnd,
  companions: 0,
  hotelLevel: props.trip.hotelLevel,
}))

function handleFormUpdate(form: { destination: string; travelStart: string; travelEnd: string; hotelLevel: string }) {
  emit('form-update', props.trip.id, form)
}

function handleMoveInvoice(invoiceId: string, targetTripId: string) {
  emit('move', invoiceId, targetTripId || null)
}
</script>
```

- [ ] **步骤 2：类型检查**

运行：`npx vue-tsc --noEmit`
预期：无错误（组件暂未被引用，仅做独立类型校验）。

- [ ] **步骤 3：Commit**

```bash
git add src/components/TripCard.vue
git commit -m "feat(ui): 新增 TripCard 单趟出差卡片组件"
```

---

## 任务 5：ExportView 分趟改造

**文件：**
- 修改：`src/views/ExportView.vue`（整体重写）

- [ ] **步骤 1：重写 ExportView**

将 `src/views/ExportView.vue` 全部内容替换为：

```vue
<template>
  <div class="max-w-4xl mx-auto">
    <h2 class="text-2xl font-bold mb-6">导出报销表</h2>

    <div v-if="matchStore.matches.length === 0" class="text-center py-12 text-gray-400">
      请先在匹配页面完成发票与账单的匹配
    </div>

    <template v-else>
      <!-- 匹配摘要 -->
      <div class="bg-white rounded-lg border p-4 shadow-sm mb-6">
        <div class="grid grid-cols-3 gap-4 text-center">
          <div>
            <p class="text-2xl font-bold text-blue-600">{{ matchStore.matches.length }}</p>
            <p class="text-sm text-gray-500">已匹配</p>
          </div>
          <div>
            <p class="text-2xl font-bold text-orange-500">{{ matchStore.unmatchedInvoices.length }}</p>
            <p class="text-sm text-gray-500">未匹配发票</p>
          </div>
          <div>
            <p class="text-2xl font-bold text-gray-400">{{ matchStore.unmatchedPayments.length }}</p>
            <p class="text-sm text-gray-500">未匹配支付</p>
          </div>
        </div>
      </div>

      <!-- 分趟工具栏：存在待调整票据时提供出发城市重匹配 -->
      <div v-if="hasUnassignedTickets"
           class="bg-white rounded-lg border p-4 shadow-sm mb-6 flex flex-wrap items-center gap-3">
        <div class="flex items-center gap-2">
          <label class="text-sm text-gray-600">出发城市</label>
          <input v-model="originInput" class="w-32 border rounded px-2 py-1.5 text-sm focus:outline-none focus:border-blue-500"
                 placeholder="如：长沙" />
        </div>
        <button @click="handleResegment"
                class="px-3 py-2 rounded bg-green-500 text-white text-sm hover:bg-green-600 transition-colors">
          重新匹配行程
        </button>
        <button @click="handleResetAuto"
                class="px-3 py-2 rounded border text-sm hover:bg-gray-50 transition-colors">
          恢复自动分趟
        </button>
        <span v-if="matchStore.segmentOrigin" class="text-xs text-gray-400">
          当前按出发城市「{{ matchStore.segmentOrigin }}」分组
        </span>
      </div>

      <!-- 分趟列表 -->
      <div class="space-y-6 mb-6">
        <TripCard
          v-for="(trip, idx) in matchStore.trips"
          :key="trip.id"
          :trip="trip"
          :index="idx + 1"
          :other-trips="otherTrips(trip)"
          @move="handleMove"
          @form-update="handleTripFormUpdate"
          @preview="previewTrip(trip)"
        />
      </div>

      <!-- 待调整区 -->
      <div v-if="matchStore.unassigned.length" class="bg-orange-50 border border-orange-200 rounded-lg p-4 mb-6">
        <h3 class="text-sm font-medium text-orange-700 mb-1">待调整（{{ matchStore.unassigned.length }}）</h3>
        <p class="text-xs text-orange-500 mb-3">
          以下发票无法自动归入某趟出差（票据未配对成功或日期在行程之外），可移入某趟；票据可「新建出差」。
        </p>
        <div class="space-y-2">
          <div v-for="m in matchStore.unassigned" :key="m.invoice_id"
               class="flex items-center gap-2 bg-white rounded px-3 py-2 border border-orange-100 text-sm flex-wrap">
            <span class="w-20 shrink-0 text-xs font-medium" :class="getCategoryBadgeClass(m.invoice.category)">
              {{ CATEGORY_LABELS[m.invoice.category] }}
            </span>
            <span class="text-gray-500 truncate flex-1">{{ m.invoice.seller_name || m.invoice.invoice_number || m.invoice.id }}</span>
            <span class="text-gray-500 shrink-0">{{ m.invoice.travel_date || m.invoice.date }}</span>
            <span class="text-gray-800 shrink-0">¥{{ m.invoice.amount.toFixed(2) }}</span>
            <button v-if="isTicket(m.invoice)" @click="handleCreateTrip(m)"
                    class="text-xs px-2 py-1 rounded bg-blue-500 text-white hover:bg-blue-600 transition-colors shrink-0">
              新建出差
            </button>
            <select @change="handleMove(m.invoice_id, ($event.target as HTMLSelectElement).value)"
                    class="text-xs border rounded px-1 py-0.5 shrink-0">
              <option value="" disabled selected>移到出差...</option>
              <option v-for="t in matchStore.trips" :key="t.id" :value="t.id">出差 {{ t.destination || t.id }}</option>
            </select>
          </div>
        </div>
      </div>

      <!-- 报销单预览 -->
      <div v-if="matchStore.reimbursementHtml" class="border rounded-lg overflow-hidden mb-6">
        <div class="bg-gray-100 px-4 py-2 text-sm text-gray-600">
          <span>报销单预览{{ previewingTrip ? ' · ' + (previewingTrip.destination || previewingTrip.id) : '' }}</span>
        </div>
        <iframe
          :srcdoc="matchStore.reimbursementHtml"
          class="w-full"
          style="min-height: 600px; border: none;"
          title="报销单预览"
        />
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useMatchStore } from '../stores/match'
import TripCard from '../components/TripCard.vue'
import type { Invoice, MatchResult, Trip } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass } from '../utils/category'

const matchStore = useMatchStore()

const originInput = ref('')
const previewingTrip = ref<Trip | null>(null)

function isTicket(invoice: Invoice) {
  return invoice.category === 'Train' || invoice.category === 'Flight'
}

const hasUnassignedTickets = computed(() =>
  matchStore.unassigned.some(m => isTicket(m.invoice))
)

function otherTrips(trip: Trip): Trip[] {
  return matchStore.trips.filter(t => t.id !== trip.id)
}

async function handleResegment() {
  const origin = originInput.value.trim()
  if (!origin) {
    alert('请先输入出发城市')
    return
  }
  try {
    matchStore.segmentOrigin = origin
    await matchStore.resegment(matchStore.matches, origin)
  } catch (e) {
    console.error('重新匹配失败:', e)
    alert('重新匹配失败: ' + e)
  }
}

async function handleResetAuto() {
  originInput.value = ''
  try {
    matchStore.segmentOrigin = ''
    await matchStore.resegment(matchStore.matches, '')
  } catch (e) {
    console.error('恢复自动分趟失败:', e)
    alert('恢复自动分趟失败: ' + e)
  }
}

function handleMove(invoiceId: string, targetTripId: string | null) {
  matchStore.moveToTrip(invoiceId, targetTripId)
}

function handleTripFormUpdate(tripId: string, form: { destination: string; travelStart: string; travelEnd: string; hotelLevel: string }) {
  const trip = matchStore.trips.find(t => t.id === tripId)
  if (!trip) return
  trip.destination = form.destination
  trip.travelStart = form.travelStart
  trip.travelEnd = form.travelEnd
  trip.hotelLevel = form.hotelLevel
}

function handleCreateTrip(match: MatchResult) {
  matchStore.createTripFromTicket(match)
}

async function previewTrip(trip: Trip) {
  previewingTrip.value = trip
  try {
    await matchStore.renderReimbursementHtml(
      {
        name: '',
        department: '',
        destination: trip.destination,
        travelStart: trip.travelStart,
        travelEnd: trip.travelEnd,
        companions: 0,
        hotelLevel: trip.hotelLevel,
      },
      trip.matches,
    )
  } catch (e) {
    console.error('预览失败:', e)
    alert('预览失败: ' + e)
  }
}
</script>
```

- [ ] **步骤 2：构建检查**

运行：`npm run build:check`
预期：`vue-tsc --noEmit` 与 `vite build` 均成功（无类型错误）。

- [ ] **步骤 3：运行全部前端测试**

运行：`npm test`
预期：全部 PASS（含新 store 分趟测试与既有测试）。

- [ ] **步骤 4：Commit**

```bash
git add src/views/ExportView.vue
git commit -m "feat(ui): 导出页改造为分趟预览与逐趟导出"
```

---

## 任务 6：整体验证

- [ ] **步骤 1：后端全量测试**

运行：`cargo test -p invoice-reimbursement 2>&1 | Select-String -Pattern "test result|FAILED"`
预期：全部 `test result: ok`，无 FAILED。

- [ ] **步骤 2：前端类型与构建**

运行：`npm run build:check`
预期：成功。

- [ ] **步骤 3：手动冒烟验证**

运行：`npm run tauri dev`，按以下路径验证：

1. 导入两张往返火车票 PDF + 一张该行程日期内的餐费发票 → 自动匹配 → 导出页出现 1 趟出差卡片（目的地=去程到达城市，起止=往返日期），餐费发票已归入
2. 再导入另一次往返的机票/火车票 + 酒店发票 → 重新自动匹配 → 出现 2 趟
3. 移除某趟返程票场景：导入单程票（有去无回）→ 自动分趟后该票在"待调整"区 → 点"新建出差"生成单趟
4. 缺中间程场景：两张票 `长沙→武汉`、`北京→长沙` → 待调整 → 输入出发城市"长沙" → 点"重新匹配行程" → 自动合成 1 趟，目的地=北京
5. 每趟卡片上点"预览"→ 下方 iframe 显示该趟报销单；点各导出按钮 → 每趟分别生成文件
6. 发票明细折叠区内的下拉可把发票移到其他趟或待调整
7. 仅导入餐费/酒店等无票据发票 → 导出页仍显示单趟（destination 留空可手填），与原功能一致

- [ ] **步骤 4：收尾 commit（如有冒烟修复）**

如有改动则提交；无改动则无需额外提交。

---

## 自检

**规格覆盖度：**
- §1.1 票据筛选 → 任务 1 `collect_tickets` ✅
- §1.2 链式贪心成组（自动模式）→ 任务 1 `segment_auto` ✅
- §1.3 非票据发票窗口归入 → 任务 1 `effective_date` + `assign_by_date` ✅
- §1.4 指定 origin 全量重分组 → 任务 1 `segment_with_origin` + 任务 5 工具栏/`segmentOrigin` ✅
- §1.5 输出结构 → 任务 1 `TripGroup`/`SegmentResult` ✅
- §2 前端状态 → 任务 3（trips/unassigned/segmentOrigin/resegment/moveToTrip/createTripFromTicket）✅
- §3.1 分趟区（含工具栏）→ 任务 4/5 ✅
- §3.2 待调整区 → 任务 5 ✅
- §3.3 无票据兜底单趟 → 任务 3 store 兜底 ✅
- §4 导出复用（零后端导出改动）→ 任务 4 `ExportButton` 传子集 + 任务 5 预览传子集 ✅
- §5 测试用例（1-11 + origin 4 例）→ 任务 1 的 12 个 Rust 测试 + 任务 3 的 5 个 Vitest ✅
- §6 边界 → 已在算法注释与测试中覆盖 ✅

**占位符扫描：** 无"待定/TODO/后续实现"；每个代码步骤含完整代码。✅

**类型一致性：** `Trip`（frontend）字段与 `TripGroupDto`/store 映射一致；`segment_trips` 命令签名（`match_results: Vec<MatchResult>`, `origin: Option<String>`）与前端 `invoke('segment_trips', { matchResults, origin })` 参数名经 Tauri camelCase→snake_case 转换匹配；`renderReimbursementHtml` 第二参数 `matchesOverride` 在任务 5 预览调用处一致。✅
