use crate::models::invoice::{Invoice, InvoiceCategory};

#[derive(Debug, Clone, PartialEq)]
pub enum MatchingStrategy {
    StrictAmountOnly,
    AmountWithMerchant,
    AmountWithTime,
    MultiDimensional,
    OneToMany,
    FuzzyMatching,
}

pub struct StrategySelector;

impl StrategySelector {
    pub fn new() -> Self {
        Self
    }

    pub fn select(invoice: &Invoice, payment_count: usize) -> MatchingStrategy {
        if invoice.category == InvoiceCategory::CityTransport && !invoice.itineraries.is_empty() {
            return MatchingStrategy::OneToMany;
        }

        if !invoice.seller_name.is_empty() {
            return MatchingStrategy::AmountWithMerchant;
        }

        if payment_count < 50 {
            return MatchingStrategy::MultiDimensional;
        }

        MatchingStrategy::MultiDimensional
    }

    pub fn select_with_context(
        invoice: &Invoice,
        payment_count: usize,
        has_merchant_info: bool,
        time_accuracy: TimeAccuracy,
    ) -> MatchingStrategy {
        if invoice.category == InvoiceCategory::CityTransport && !invoice.itineraries.is_empty() {
            return MatchingStrategy::OneToMany;
        }

        if has_merchant_info && !invoice.seller_name.is_empty() {
            return MatchingStrategy::AmountWithMerchant;
        }

        if time_accuracy == TimeAccuracy::Precise {
            return MatchingStrategy::AmountWithTime;
        }

        if payment_count < 20 {
            return MatchingStrategy::StrictAmountOnly;
        }

        if payment_count > 200 {
            return MatchingStrategy::FuzzyMatching;
        }

        MatchingStrategy::MultiDimensional
    }

    pub fn get_strategy_description(strategy: &MatchingStrategy) -> &'static str {
        match strategy {
            MatchingStrategy::StrictAmountOnly => "仅金额精确匹配，适用于高精度场景",
            MatchingStrategy::AmountWithMerchant => "金额+商户名称匹配，提高准确率",
            MatchingStrategy::AmountWithTime => "金额+时间匹配，适用于时间敏感场景",
            MatchingStrategy::MultiDimensional => "多维度综合评分匹配，平衡准确性和覆盖率",
            MatchingStrategy::OneToMany => "一对多匹配，适用于打车行程单等场景",
            MatchingStrategy::FuzzyMatching => "模糊匹配，适用于低置信度场景",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeAccuracy {
    Precise,
    Approximate,
    Unknown,
}

impl Default for StrategySelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{InvoiceSource, Itinerary};
    #[allow(unused_imports)]
    use crate::models::payment::PaymentSource;
    use chrono::NaiveDate;

    fn make_invoice(id: &str, category: InvoiceCategory, seller: &str, has_itineraries: bool) -> Invoice {
        let itineraries = if has_itineraries {
            vec![Itinerary {
                date_time: "2025-01-15 10:00".to_string(),
                provider: "滴滴".to_string(),
                pickup: "A站".to_string(),
                dropoff: "B站".to_string(),
                amount: 30.0,
            }]
        } else {
            vec![]
        };

        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount: 100.0,
            seller_name: seller.to_string(),
            item_name: "Test Item".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            category,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries,
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
        }
    }

    #[test]
    fn test_select_one_to_many_for_city_transport_with_itineraries() {
        let invoice = make_invoice("inv1", InvoiceCategory::CityTransport, "滴滴出行", true);
        let strategy = StrategySelector::select(&invoice, 100);
        assert_eq!(strategy, MatchingStrategy::OneToMany);
    }

    #[test]
    fn test_select_amount_with_merchant_when_seller_exists() {
        let invoice = make_invoice("inv1", InvoiceCategory::Hotel, "如家酒店", false);
        let strategy = StrategySelector::select(&invoice, 100);
        assert_eq!(strategy, MatchingStrategy::AmountWithMerchant);
    }

    #[test]
    fn test_select_multi_dimensional_for_small_dataset() {
        let invoice = make_invoice("inv1", InvoiceCategory::Hotel, "", false);
        let strategy = StrategySelector::select(&invoice, 30);
        assert_eq!(strategy, MatchingStrategy::MultiDimensional);
    }

    #[test]
    fn test_select_multi_dimensional_default() {
        let invoice = make_invoice("inv1", InvoiceCategory::Other, "", false);
        let strategy = StrategySelector::select(&invoice, 100);
        assert_eq!(strategy, MatchingStrategy::MultiDimensional);
    }

    #[test]
    fn test_city_transport_without_itineraries() {
        let invoice = make_invoice("inv1", InvoiceCategory::CityTransport, "滴滴出行", false);
        let strategy = StrategySelector::select(&invoice, 100);
        assert_eq!(strategy, MatchingStrategy::AmountWithMerchant);
    }

    #[test]
    fn test_priority_order() {
        let invoice_taxi = make_invoice("inv1", InvoiceCategory::CityTransport, "滴滴", true);
        assert_eq!(
            StrategySelector::select(&invoice_taxi, 100),
            MatchingStrategy::OneToMany
        );

        let invoice_hotel = make_invoice("inv2", InvoiceCategory::Hotel, "如家", false);
        assert_eq!(
            StrategySelector::select(&invoice_hotel, 100),
            MatchingStrategy::AmountWithMerchant
        );

        let invoice_no_seller = make_invoice("inv3", InvoiceCategory::Meal, "", false);
        assert_eq!(
            StrategySelector::select(&invoice_no_seller, 30),
            MatchingStrategy::MultiDimensional
        );
    }

    #[test]
    fn test_select_with_context_one_to_many() {
        let invoice = make_invoice("inv1", InvoiceCategory::CityTransport, "滴滴", true);
        let strategy = StrategySelector::select_with_context(
            &invoice,
            100,
            true,
            TimeAccuracy::Precise,
        );
        assert_eq!(strategy, MatchingStrategy::OneToMany);
    }

    #[test]
    fn test_select_with_context_amount_with_time() {
        let invoice = make_invoice("inv1", InvoiceCategory::Train, "", false);
        let strategy = StrategySelector::select_with_context(
            &invoice,
            100,
            false,
            TimeAccuracy::Precise,
        );
        assert_eq!(strategy, MatchingStrategy::AmountWithTime);
    }

    #[test]
    fn test_select_with_context_strict_amount_only() {
        let invoice = make_invoice("inv1", InvoiceCategory::Other, "", false);
        let strategy = StrategySelector::select_with_context(
            &invoice,
            15,
            false,
            TimeAccuracy::Unknown,
        );
        assert_eq!(strategy, MatchingStrategy::StrictAmountOnly);
    }

    #[test]
    fn test_select_with_context_fuzzy_matching() {
        let invoice = make_invoice("inv1", InvoiceCategory::Other, "", false);
        let strategy = StrategySelector::select_with_context(
            &invoice,
            300,
            false,
            TimeAccuracy::Unknown,
        );
        assert_eq!(strategy, MatchingStrategy::FuzzyMatching);
    }

    #[test]
    fn test_get_strategy_description() {
        assert_eq!(
            StrategySelector::get_strategy_description(&MatchingStrategy::OneToMany),
            "一对多匹配，适用于打车行程单等场景"
        );

        assert_eq!(
            StrategySelector::get_strategy_description(&MatchingStrategy::AmountWithMerchant),
            "金额+商户名称匹配，提高准确率"
        );
    }

    #[test]
    fn test_time_accuracy_variants() {
        assert_eq!(TimeAccuracy::Precise, TimeAccuracy::Precise);
        assert_ne!(TimeAccuracy::Precise, TimeAccuracy::Approximate);
    }

    #[test]
    fn test_default_strategy_selector() {
        let _selector = StrategySelector::default();
        let invoice = make_invoice("inv1", InvoiceCategory::Hotel, "测试", false);
        let strategy = StrategySelector::select(&invoice, 50);
        assert_eq!(strategy, MatchingStrategy::AmountWithMerchant);
    }
}
