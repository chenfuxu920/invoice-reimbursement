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
