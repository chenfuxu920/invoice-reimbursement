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
                        invoice_ids: std::mem::take(chain),
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

/// 非票据发票的候选归口日期：任一日期命中趟窗口即归入该趟
/// - Hotel：入住日（无则开票日）
/// - CityTransport：行程单所有行程的日期（含年-less "MM-DD HH:MM"，由 datetime_util 补当年；无行程则开票日）
/// - Toll：通行时间（无则开票日）
/// - 其他：开票日
fn candidate_dates(inv: &Invoice) -> Vec<NaiveDate> {
    match inv.category {
        InvoiceCategory::Hotel => inv
            .hotel_detail
            .as_ref()
            .and_then(|h| h.check_in)
            .map(|d| vec![d])
            .unwrap_or_else(|| vec![inv.date]),
        InvoiceCategory::CityTransport => {
            let ride_dates: Vec<NaiveDate> = inv
                .itineraries
                .iter()
                .filter_map(|it| {
                    crate::parser::datetime_util::parse_datetime(&it.date_time).map(|dt| dt.date())
                })
                .collect();
            if ride_dates.is_empty() {
                vec![inv.date]
            } else {
                ride_dates
            }
        }
        InvoiceCategory::Toll => inv
            .toll_travel_time
            .map(|t| vec![t.date()])
            .unwrap_or_else(|| vec![inv.date]),
        _ => vec![inv.date],
    }
}

/// 市内交通发票的打车城市是否命中趟有效城市。
/// 优先用行程单"城市"列（精确匹配）；城市列缺失时回退到销售方/备注/上下车点子串匹配。
/// 无行程（无城市信息）时无法判断 → 放行按日期归入（兜底）。
fn city_matches_trip(inv: &Invoice, trip_cities: &HashSet<String>) -> bool {
    if trip_cities.is_empty() || inv.itineraries.is_empty() {
        return true;
    }
    let mut has_city_field = false;
    for it in &inv.itineraries {
        let c = it.city.trim();
        if !c.is_empty() {
            has_city_field = true;
            if trip_cities.contains(c) {
                return true;
            }
        }
    }
    if has_city_field {
        // 城市列均有值但不匹配 → 明确非该趟城市
        return false;
    }
    let haystacks: Vec<String> = std::iter::once(inv.seller_name.clone())
        .chain(std::iter::once(inv.item_name.clone()))
        .chain(std::iter::once(inv.remarks.clone()))
        .chain(
            inv.itineraries
                .iter()
                .flat_map(|it| [it.pickup.clone(), it.dropoff.clone()]),
        )
        .collect();
    trip_cities
        .iter()
        .any(|c| haystacks.iter().any(|h| h.contains(c.as_str())))
}

/// 非票据发票按日期落入趟窗口 [start, end]：候选日期任一命中即归入，跨窗口取最早开始；
/// 全部在窗口外进待调整。
/// 市内交通（滴滴/高德等）额外要求打车城市 ∈ 该趟票据链城市，否则剔除进待调整。
/// 三字段齐全的票据（在 ticket_ids 中）只随链归属，绝不按窗口归入；
/// 缺字段的 Train/Flight（未被收集为票据）与普通发票一样按窗口归入。
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

    // 每趟有效城市 = 票据链上所有出发/到达城市（如 长沙→武汉→北京→长沙 → {长沙,武汉,北京}）
    let ticket_by_id: HashMap<&str, &Ticket> = tickets
        .iter()
        .map(|t| (t.invoice_id.as_str(), t))
        .collect();
    let trip_cities: Vec<HashSet<String>> = trips
        .iter()
        .map(|trip| {
            trip.ticket_ids
                .iter()
                .filter_map(|id| ticket_by_id.get(id.as_str()))
                .flat_map(|t| [t.departure.clone(), t.arrival.clone()])
                .collect()
        })
        .collect();

    let mut assigned: HashMap<&str, usize> = HashMap::new();
    let mut unassigned_set: HashSet<String> = unassigned.iter().cloned().collect();

    for m in match_results {
        let inv = &m.invoice;
        if ticket_ids.contains(inv.id.as_str()) {
            continue;
        }
        let dates = candidate_dates(inv);
        let mut best: Option<usize> = None;
        for (i, (s, e)) in windows.iter().enumerate() {
            if dates.iter().any(|d| *d >= *s && *d <= *e) {
                if inv.category == InvoiceCategory::CityTransport
                    && !city_matches_trip(inv, &trip_cities[i])
                {
                    continue;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{HotelDetail, InvoiceSource};
    use crate::models::match_result::MatchType;
    use chrono::{Datelike, NaiveDateTime};

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
            check_in: Some(NaiveDate::parse_from_str("2026-05-20", "%Y-%m-%d").unwrap()),
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
        ct.itineraries = vec![crate::models::invoice::Itinerary { city: String::new(),
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

    // 回归：行程单时间可能是年-less 格式（"04-25 08:48"），旧实现按 "%Y-%m-%d"
    // 解析失败 → 回退开票日期（行程之后）→ 滴滴落不进趟窗口
    #[test]
    fn test_auto_city_transport_yearless_itinerary_date() {
        let year = chrono::Local::now().year();
        let y = year.to_string();
        let mut ct = inv("c1", InvoiceCategory::CityTransport, "2026-06-10");
        ct.itineraries = vec![crate::models::invoice::Itinerary { city: String::new(),
            date_time: "04-25 08:48".to_string(),
            provider: "滴滴".to_string(),
            pickup: "长沙站".to_string(),
            dropoff: "国贸".to_string(),
            amount: 35.0,
            incomplete_fields: vec![],
        }];
        let results = vec![
            mr(ticket("t1", "长沙", "上海", &format!("{y}-04-25"))),
            mr(ticket("t2", "上海", "长沙", &format!("{y}-04-27"))),
            mr(ct),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips[0].invoice_ids.len(), 3);
        assert!(seg.trips[0].invoice_ids.iter().any(|id| id == "c1"));
        assert!(seg.unassigned_ids.is_empty());
    }

    // 回归：滴滴发票多条行程跨多天，首条行程在窗口外、后续行程在窗口内 → 仍应归入
    #[test]
    fn test_auto_city_transport_uses_any_itinerary_date() {
        let mut ct = inv("c1", InvoiceCategory::CityTransport, "2026-06-10");
        ct.seller_name = "滴滴出行".to_string();
        ct.itineraries = vec![
            crate::models::invoice::Itinerary { city: String::new(),
                date_time: "2026-06-01 08:00".to_string(),
                provider: "滴滴".to_string(),
                pickup: "杭州站".to_string(),
                dropoff: "B".to_string(),
                amount: 30.0,
                incomplete_fields: vec![],
            },
            crate::models::invoice::Itinerary { city: String::new(),
                date_time: "2026-05-21 09:00".to_string(),
                provider: "滴滴".to_string(),
                pickup: "上海站".to_string(),
                dropoff: "D".to_string(),
                amount: 40.0,
                incomplete_fields: vec![],
            },
        ];
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

    // 滴滴全部行程都在所有趟窗口外 → 待调整
    #[test]
    fn test_auto_city_transport_outside_windows_unassigned() {
        let mut ct = inv("c1", InvoiceCategory::CityTransport, "2026-06-10");
        ct.itineraries = vec![crate::models::invoice::Itinerary { city: String::new(),
            date_time: "2026-06-10 08:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "A".to_string(),
            dropoff: "B".to_string(),
            amount: 30.0,
            incomplete_fields: vec![],
        }];
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "上海", "长沙", "2026-05-22")),
            mr(ct),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 1);
        assert_eq!(seg.unassigned_ids, vec!["c1".to_string()]);
    }

    // 回归：滴滴行程单有"城市"信息（上下车点/销售方含城市名），
    // 城市不在该趟票据链城市内 → 即使日期在窗口内也剔除（进待调整）
    #[test]
    fn test_auto_city_transport_wrong_city_unassigned() {
        let mut ct = inv("c1", InvoiceCategory::CityTransport, "2026-06-10");
        ct.seller_name = "滴滴出行".to_string();
        ct.itineraries = vec![crate::models::invoice::Itinerary { city: String::new(),
            date_time: "2026-05-21 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "武汉站".to_string(),
            dropoff: "江汉路".to_string(),
            amount: 30.0,
            incomplete_fields: vec![],
        }];
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "上海", "长沙", "2026-05-22")),
            mr(ct),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 1);
        assert_eq!(seg.unassigned_ids, vec!["c1".to_string()]);
    }

    // 城市命中（上下车点含链上城市）且日期在窗口内 → 归入
    #[test]
    fn test_auto_city_transport_matching_city_assigned() {
        let mut ct = inv("c1", InvoiceCategory::CityTransport, "2026-06-10");
        ct.seller_name = "滴滴出行".to_string();
        ct.itineraries = vec![crate::models::invoice::Itinerary { city: String::new(),
            date_time: "2026-05-21 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "上海虹桥站".to_string(),
            dropoff: "人民广场".to_string(),
            amount: 30.0,
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

    // 长出差链：长沙→武汉→北京，武汉的滴滴发票应归入该趟（武汉为链上城市）
    #[test]
    fn test_auto_city_transport_chain_city_assigned() {
        let mut ct = inv("c1", InvoiceCategory::CityTransport, "2026-06-10");
        ct.seller_name = "滴滴出行".to_string();
        ct.itineraries = vec![crate::models::invoice::Itinerary {
            date_time: "2026-05-21 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "武汉站".to_string(),
            dropoff: "光谷".to_string(),
            amount: 30.0,
            incomplete_fields: vec![],
            city: String::new(),
        }];
        let results = vec![
            mr(ticket("t1", "长沙", "武汉", "2026-05-20")),
            mr(ticket("t2", "武汉", "北京", "2026-05-21")),
            mr(flight("t3", "北京", "长沙", "2026-05-23")),
            mr(ct),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips[0].invoice_ids.len(), 4);
        assert!(seg.trips[0].invoice_ids.iter().any(|id| id == "c1"));
        assert!(seg.unassigned_ids.is_empty());
    }

    // 行程单"城市"列：城市列命中趟城市 → 即使上下车点为纯地名也归入
    #[test]
    fn test_city_column_matching_assigned() {
        let mut ct = inv("c1", InvoiceCategory::CityTransport, "2026-06-10");
        ct.seller_name = "滴滴出行".to_string();
        ct.itineraries = vec![crate::models::invoice::Itinerary {
            date_time: "2026-05-21 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "江汉路".to_string(),
            dropoff: "光谷".to_string(),
            amount: 30.0,
            incomplete_fields: vec![],
            city: "武汉".to_string(),
        }];
        let results = vec![
            mr(ticket("t1", "长沙", "武汉", "2026-05-20")),
            mr(ticket("t2", "武汉", "北京", "2026-05-21")),
            mr(flight("t3", "北京", "长沙", "2026-05-23")),
            mr(ct),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips[0].invoice_ids.len(), 4);
        assert!(seg.trips[0].invoice_ids.iter().any(|id| id == "c1"));
        assert!(seg.unassigned_ids.is_empty());
    }

    // 行程单"城市"列：城市列不匹配该趟 → 剔除进待调整
    #[test]
    fn test_city_column_mismatch_unassigned() {
        let mut ct = inv("c1", InvoiceCategory::CityTransport, "2026-06-10");
        ct.seller_name = "滴滴出行".to_string();
        ct.itineraries = vec![crate::models::invoice::Itinerary {
            date_time: "2026-05-21 09:00".to_string(),
            provider: "滴滴".to_string(),
            pickup: "人民广场".to_string(),
            dropoff: "外滩".to_string(),
            amount: 30.0,
            incomplete_fields: vec![],
            city: "上海".to_string(),
        }];
        let results = vec![
            mr(ticket("t1", "长沙", "武汉", "2026-05-20")),
            mr(ticket("t2", "武汉", "北京", "2026-05-21")),
            mr(flight("t3", "北京", "长沙", "2026-05-23")),
            mr(ct),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 1);
        assert_eq!(seg.unassigned_ids, vec!["c1".to_string()]);
    }

    #[test]
    fn test_empty_input() {
        let seg = segment_trips(&[], None);
        assert!(seg.trips.is_empty());
        assert!(seg.unassigned_ids.is_empty());

        let seg = segment_trips(&[], Some("长沙"));
        assert!(seg.trips.is_empty());
        assert!(seg.unassigned_ids.is_empty());
    }

    #[test]
    fn test_auto_toll_uses_travel_time() {
        let mut toll = inv("f1", InvoiceCategory::Toll, "2026-05-25");
        toll.toll_travel_time =
            Some(NaiveDateTime::parse_from_str("2026-05-21 10:06:04", "%Y-%m-%d %H:%M:%S").unwrap());
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "上海", "长沙", "2026-05-22")),
            mr(toll),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 1);
        assert!(seg.trips[0].invoice_ids.iter().any(|id| id == "f1"));
        assert!(seg.unassigned_ids.is_empty());
    }

    #[test]
    fn test_auto_city_transport_fallback_to_invoice_date() {
        let ct = inv("c1", InvoiceCategory::CityTransport, "2026-05-21");
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "上海", "长沙", "2026-05-22")),
            mr(ct),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 1);
        assert!(seg.trips[0].invoice_ids.iter().any(|id| id == "c1"));
        assert!(seg.unassigned_ids.is_empty());
    }

    #[test]
    fn test_auto_hotel_fallback_to_invoice_date() {
        let hotel = inv("h1", InvoiceCategory::Hotel, "2026-05-21");
        let results = vec![
            mr(ticket("t1", "长沙", "上海", "2026-05-20")),
            mr(ticket("t2", "上海", "长沙", "2026-05-22")),
            mr(hotel),
        ];
        let seg = segment_trips(&results, None);
        assert_eq!(seg.trips.len(), 1);
        assert!(seg.trips[0].invoice_ids.iter().any(|id| id == "h1"));
        assert!(seg.unassigned_ids.is_empty());
    }
}
