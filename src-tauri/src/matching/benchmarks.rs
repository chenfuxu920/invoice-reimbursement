use crate::models::invoice::{Invoice, InvoiceCategory, InvoiceSource, Itinerary};
use crate::models::payment::{PaymentRecord, PaymentSource};
use chrono::NaiveDate;

pub fn generate_test_invoices(count: usize) -> Vec<Invoice> {
    let categories = vec![
        InvoiceCategory::Hotel,
        InvoiceCategory::CityTransport,
        InvoiceCategory::Meal,
        InvoiceCategory::Flight,
        InvoiceCategory::Train,
        InvoiceCategory::Other,
    ];

    let merchants = vec![
        ("如家酒店", "住宿服务"),
        ("汉庭酒店", "住宿费"),
        ("滴滴出行", "网约车服务"),
        ("高德打车", "交通服务"),
        ("肯德基", "餐饮服务"),
        ("麦当劳", "餐饮"),
        ("中国国航", "机票代理"),
        ("12306", "铁路客票"),
    ];

    let mut invoices = Vec::new();
    for i in 0..count {
        let category_idx = i % categories.len();
        let merchant_idx = i % merchants.len();
        let (seller, item) = merchants[merchant_idx];
        let category = categories[category_idx].clone();

        let amount = match category {
            InvoiceCategory::Hotel => 200.0 + (i as f64 % 500.0),
            InvoiceCategory::CityTransport => 20.0 + (i as f64 % 80.0),
            InvoiceCategory::Meal => 30.0 + (i as f64 % 100.0),
            InvoiceCategory::Flight => 500.0 + (i as f64 % 2000.0),
            InvoiceCategory::Insurance => 50.0 + (i as f64 % 200.0),
            InvoiceCategory::Train => 100.0 + (i as f64 % 500.0),
            InvoiceCategory::TicketChange => 50.0 + (i as f64 % 200.0),
            InvoiceCategory::Toll => 10.0 + (i as f64 % 50.0),
            InvoiceCategory::Other => 50.0 + (i as f64 % 200.0),
        };

        let itineraries = if category == InvoiceCategory::CityTransport {
            vec![Itinerary {
                city: String::new(),
                date_time: format!("2025-01-{:02} 09:00", (i % 28) + 1),
                provider: if i % 2 == 0 { "滴滴" } else { "高德" }.to_string(),
                pickup: "起点".to_string(),
                dropoff: "终点".to_string(),
                amount: amount,
                incomplete_fields: vec![],
            }]
        } else {
            vec![]
        };

        invoices.push(Invoice {
            id: format!("inv-{}", i),
            invoice_number: format!("INV-{:06}", i),
            amount,
            seller_name: seller.to_string(),
            item_name: item.to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, ((i % 28) + 1) as u32).unwrap(),
            travel_date: None,
            category,
            source: InvoiceSource::Link(format!("http://example.com/invoice/{}", i)),
            itineraries,
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: None,
        });
    }

    invoices
}

pub fn generate_test_payments(count: usize) -> Vec<PaymentRecord> {
    let merchants = vec![
        ("如家快捷酒店", "住宿"),
        ("汉庭酒店", "酒店"),
        ("滴滴出行科技有限公司", "交通"),
        ("高德软件有限公司", "出行"),
        ("肯德基餐厅", "餐饮"),
        ("麦当劳餐厅", "快餐"),
        ("中国国际航空", "航空"),
        ("铁路12306", "火车票"),
    ];

    let mut payments = Vec::new();
    for i in 0..count {
        let merchant_idx = i % merchants.len();
        let (merchant, category) = merchants[merchant_idx];

        let amount = match merchant_idx {
            0 | 1 => 200.0 + (i as f64 % 500.0),
            2 | 3 => 20.0 + (i as f64 % 80.0),
            4 | 5 => 30.0 + (i as f64 % 100.0),
            6 => 500.0 + (i as f64 % 2000.0),
            7 => 100.0 + (i as f64 % 500.0),
            _ => 50.0 + (i as f64 % 200.0),
        };

        let day = (i % 28) + 1;
        let hour = (i % 12) + 8;

        payments.push(PaymentRecord {
            id: format!("pay-{}", i),
            transaction_id: format!("TXN-{:08}", i),
            transaction_time: format!("2025-01-{:02} {:02}:00", day, hour),
            amount,
            original_amount: amount,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: merchant.to_string(),
            source: if i % 2 == 0 {
                PaymentSource::Wechat
            } else {
                PaymentSource::Alipay
            },
            category: category.to_string(),
            payment_method: String::new(),
        });
    }

    payments
}

#[allow(dead_code)]
fn generate_matching_data(
    invoice_count: usize,
    payment_count: usize,
) -> (Vec<Invoice>, Vec<PaymentRecord>) {
    let invoices = generate_test_invoices(invoice_count);
    let mut payments = generate_test_payments(payment_count);

    for (i, invoice) in invoices.iter().enumerate() {
        if i < payments.len() {
            payments[i].amount = invoice.amount;
            payments[i].transaction_time = format!("{} 12:00", invoice.date);
        }
    }

    (invoices, payments)
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use crate::matching::BatchMatchOptimizer;
    use crate::models::match_result::MatchType;
    use std::time::Instant;

    fn print_performance_metrics(
        result: &super::super::batch_optimizer::BatchMatchResult,
        duration: std::time::Duration,
    ) {
        let total_items = result.matched.len() + result.unmatched_invoices.len();
        let avg_duration = if total_items > 0 {
            duration / total_items as u32
        } else {
            duration
        };

        println!("========================================");
        println!("Performance Metrics:");
        println!("  Matching time: {:?}", duration);
        println!("  Total matched: {}", result.matched.len());
        println!("  Unmatched invoices: {}", result.unmatched_invoices.len());
        println!("  Unmatched payments: {}", result.unmatched_payments.len());
        println!("  Match rate: {:.2}%", result.match_rate() * 100.0);
        println!(
            "  High confidence matches: {}",
            result.high_confidence_matches().len()
        );
        println!("  Average match time: {:?}", avg_duration);
        println!("========================================");
    }

    #[test]
    fn test_small_scale_performance() {
        let (invoices, payments) = generate_matching_data(10, 100);

        let optimizer = BatchMatchOptimizer::new();

        let start = Instant::now();
        let result = optimizer.batch_match(&invoices, &payments);
        let duration = start.elapsed();

        print_performance_metrics(&result, duration);

        assert!(duration.as_millis() < 100);
        assert!(result.matched.len() >= 8);
    }

    #[test]
    fn test_medium_scale_performance() {
        let (invoices, payments) = generate_matching_data(100, 1000);

        let optimizer = BatchMatchOptimizer::new();

        let start = Instant::now();
        let result = optimizer.batch_match(&invoices, &payments);
        let duration = start.elapsed();

        print_performance_metrics(&result, duration);

        assert!(
            duration.as_millis() < 1000,
            "Expected < 1000ms, got {:?}",
            duration
        );
        assert!(
            result.matched.len() > 80,
            "Expected > 80 matches, got {}",
            result.matched.len()
        );
    }

    #[test]
    #[ignore]
    fn test_large_scale_performance() {
        let (invoices, payments) = generate_matching_data(1000, 10000);

        let optimizer = BatchMatchOptimizer::new();

        let start = Instant::now();
        let result = optimizer.batch_match(&invoices, &payments);
        let duration = start.elapsed();

        print_performance_metrics(&result, duration);

        assert!(duration.as_millis() < 60000);
        assert!(result.matched.len() > 900);
    }

    #[test]
    fn test_extreme_case_payments_less_than_invoices() {
        let (invoices, payments) = generate_matching_data(100, 50);

        let optimizer = BatchMatchOptimizer::new();

        let start = Instant::now();
        let result = optimizer.batch_match(&invoices, &payments);
        let duration = start.elapsed();

        print_performance_metrics(&result, duration);

        assert!(duration.as_millis() < 500);
        assert!(result.matched.len() + result.unmatched_invoices.len() >= 90);
        assert!(result.unmatched_invoices.len() >= 40);
    }

    #[test]
    fn test_one_to_many_matching_performance() {
        let mut invoices = Vec::new();
        for i in 0..50 {
            invoices.push(Invoice {
                id: format!("inv-{}", i),
                invoice_number: format!("INV-{:06}", i),
                amount: 100.0,
                seller_name: "滴滴出行".to_string(),
                item_name: "市内交通".to_string(),
                date: NaiveDate::from_ymd_opt(2025, 1, ((i % 28) + 1) as u32).unwrap(),
                travel_date: None,
                category: InvoiceCategory::CityTransport,
                source: InvoiceSource::Link(format!("http://example.com/invoice/{}", i)),
                itineraries: vec![Itinerary {
                    city: String::new(),
                    date_time: format!("2025-01-{:02} 09:00", (i % 28) + 1),
                    provider: "滴滴".to_string(),
                    pickup: "起点".to_string(),
                    dropoff: "终点".to_string(),
                    amount: 100.0,
                    incomplete_fields: vec![],
                }],
                itinerary_file: None,
                remarks: String::new(),
                hotel_detail: None,
                departure_city: None,
                arrival_city: None,
                toll_travel_time: None,
            });
        }

        let mut payments = Vec::new();
        for i in 0..200 {
            payments.push(PaymentRecord {
                id: format!("pay-{}", i),
                transaction_id: format!("TXN-{:08}", i),
                transaction_time: format!("2025-01-{:02} {:02}:00", (i % 28) + 1, (i % 12) + 8),
                amount: 30.0 + (i as f64 % 40.0),
                original_amount: 30.0 + (i as f64 % 40.0),
                refund_amount: 0.0,
                discount: 0.0,
                merchant_name: "滴滴".to_string(),
                source: if i % 2 == 0 {
                    PaymentSource::Wechat
                } else {
                    PaymentSource::Alipay
                },
                category: "交通".to_string(),
                payment_method: String::new(),
            });
        }

        let optimizer = BatchMatchOptimizer::new();

        let start = Instant::now();
        let result = optimizer.batch_match(&invoices, &payments);
        let duration = start.elapsed();

        println!("========================================");
        println!("One-to-Many Matching Performance:");
        println!("  Total time: {:?}", duration);
        println!("  Total matched: {}", result.matched.len());
        println!(
            "  One-to-One matches: {}",
            result
                .matched
                .iter()
                .filter(|m| matches!(m.match_type, MatchType::OneToOne))
                .count()
        );
        println!(
            "  One-to-Many matches: {}",
            result
                .matched
                .iter()
                .filter(|m| matches!(m.match_type, MatchType::OneToMany))
                .count()
        );
        println!("========================================");

        assert!(duration.as_millis() < 2000);
        assert!(result.matched.len() > 0);
    }

    #[test]
    fn test_performance_regression() {
        let (invoices, payments) = generate_matching_data(100, 1000);

        let optimizer = BatchMatchOptimizer::new();

        let mut durations = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let _ = optimizer.batch_match(&invoices, &payments);
            durations.push(start.elapsed());
        }

        let avg_duration = durations.iter().sum::<std::time::Duration>() / 5;
        let max_duration = durations.iter().max().unwrap();

        println!("========================================");
        println!("Performance Regression Test:");
        println!("  Runs: 5");
        println!("  Average time: {:?}", avg_duration);
        println!("  Max time: {:?}", max_duration);
        println!("  Min time: {:?}", durations.iter().min().unwrap());
        println!("========================================");

        assert!(
            max_duration.as_millis() < 2000,
            "Expected < 2000ms, got {:?}",
            max_duration
        );
    }

    #[test]
    fn test_match_quality() {
        let (invoices, payments) = generate_matching_data(100, 1000);

        let optimizer = BatchMatchOptimizer::new();
        let result = optimizer.batch_match(&invoices, &payments);

        let high_confidence = result.high_confidence_matches();
        let medium_confidence: Vec<_> = result
            .matched
            .iter()
            .filter(|m| m.confidence >= 0.5 && m.confidence < 0.7)
            .collect();
        let low_confidence: Vec<_> = result
            .matched
            .iter()
            .filter(|m| m.confidence < 0.5)
            .collect();

        println!("========================================");
        println!("Match Quality Analysis:");
        println!("  High confidence (>=0.7): {}", high_confidence.len());
        println!("  Medium confidence (0.5-0.7): {}", medium_confidence.len());
        println!("  Low confidence (<0.5): {}", low_confidence.len());
        println!("  Total matches: {}", result.matched.len());
        println!("========================================");

        assert!(high_confidence.len() > 80);
        assert!(low_confidence.len() < 5);
    }

    #[test]
    #[ignore]
    fn test_memory_efficiency() {
        let (invoices, payments) = generate_matching_data(500, 5000);

        let optimizer = BatchMatchOptimizer::new();

        let start = Instant::now();
        let result = optimizer.batch_match(&invoices, &payments);
        let duration = start.elapsed();

        println!("========================================");
        println!("Memory Efficiency Test:");
        println!("  Processing time: {:?}", duration);
        println!(
            "  Invoices: {}, Payments: {}",
            invoices.len(),
            payments.len()
        );
        println!("  Matches: {}", result.matched.len());
        println!("========================================");

        assert!(duration.as_millis() < 10000);
    }

    #[test]
    #[ignore]
    fn test_stress_test() {
        let (invoices, payments) = generate_matching_data(2000, 20000);

        let optimizer = BatchMatchOptimizer::new();

        let start = Instant::now();
        let result = optimizer.batch_match(&invoices, &payments);
        let duration = start.elapsed();

        println!("========================================");
        println!("Stress Test (2000 invoices + 20000 payments):");
        println!("  Total time: {:?}", duration);
        println!("  Matched: {}", result.matched.len());
        println!("  Match rate: {:.2}%", result.match_rate() * 100.0);
        println!("========================================");

        assert!(duration.as_secs() < 300);
    }
}

#[cfg(test)]
mod data_generator_tests {
    use super::*;

    #[test]
    fn test_generate_invoices() {
        let invoices = generate_test_invoices(10);
        assert_eq!(invoices.len(), 10);

        for invoice in &invoices {
            assert!(!invoice.id.is_empty());
            assert!(!invoice.invoice_number.is_empty());
            assert!(invoice.amount > 0.0);
            assert!(!invoice.seller_name.is_empty());
        }
    }

    #[test]
    fn test_generate_payments() {
        let payments = generate_test_payments(100);
        assert_eq!(payments.len(), 100);

        for payment in &payments {
            assert!(!payment.id.is_empty());
            assert!(!payment.transaction_id.is_empty());
            assert!(payment.amount > 0.0);
            assert!(!payment.merchant_name.is_empty());
        }
    }

    #[test]
    fn test_invoice_category_distribution() {
        let invoices = generate_test_invoices(100);
        let categories: std::collections::HashSet<_> =
            invoices.iter().map(|i| i.category.clone()).collect();

        assert!(categories.len() >= 4);
    }

    #[test]
    fn test_payment_merchant_distribution() {
        let payments = generate_test_payments(100);
        let merchants: std::collections::HashSet<_> =
            payments.iter().map(|p| p.merchant_name.clone()).collect();

        assert!(merchants.len() >= 4);
    }
}
