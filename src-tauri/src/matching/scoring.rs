use crate::models::invoice::{Invoice, InvoiceCategory};
use crate::models::payment::PaymentRecord;
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchScore {
    pub total: f64,
    pub amount_score: f64,
    pub merchant_score: f64,
    pub time_score: f64,
    pub category_score: f64,
    pub breakdown: ScoreBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub amount_diff: f64,
    pub merchant_similarity: f64,
    pub time_diff_hours: f64,
    pub category_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub amount: f64,
    pub merchant: f64,
    pub time: f64,
    pub category: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            amount: 0.4,
            merchant: 0.3,
            time: 0.2,
            category: 0.1,
        }
    }
}

pub struct MultiDimensionalScorer {
    weights: ScoringWeights,
}

impl MultiDimensionalScorer {
    pub fn new(weights: ScoringWeights) -> Self {
        Self { weights }
    }

    pub fn default_weights() -> Self {
        Self::new(ScoringWeights::default())
    }

    pub fn score(&self, invoice: &Invoice, payment: &PaymentRecord) -> MatchScore {
        // 同时比较 amount 和 total_value()，取差额更小的
        let score_total = self.score_amount(invoice.amount, payment.total_value());
        let score_amount = self.score_amount(invoice.amount, payment.amount);
        let amount_score = score_total.max(score_amount);
        let merchant_score = self.score_merchant(&invoice.seller_name, &payment.merchant_name);
        let time_score = self.score_time(&invoice.date, &payment.transaction_time);
        let category_score = self.score_category(&invoice.category, &payment.category);

        let time_diff_hours = self.calculate_time_diff_hours(&invoice.date, &payment.transaction_time);

        let total = amount_score * self.weights.amount
            + merchant_score * self.weights.merchant
            + time_score * self.weights.time
            + category_score * self.weights.category;

        MatchScore {
            total,
            amount_score,
            merchant_score,
            time_score,
            category_score,
            breakdown: ScoreBreakdown {
                amount_diff: (invoice.amount - payment.total_value()).abs()
                    .min((invoice.amount - payment.amount).abs()),
                merchant_similarity: merchant_score,
                time_diff_hours,
                category_match: category_score > 0.5,
            },
        }
    }

    pub fn score_amount(&self, invoice_amount: f64, payment_amount: f64) -> f64 {
        let diff = (invoice_amount - payment_amount).abs();
        let tolerance = 5.0;

        if diff == 0.0 {
            1.0
        } else if diff <= tolerance {
            1.0 - (diff / tolerance) * 0.5
        } else {
            0.0
        }
    }

    pub fn score_merchant(&self, invoice_seller: &str, payment_merchant: &str) -> f64 {
        if invoice_seller.is_empty() || payment_merchant.is_empty() {
            return 0.0;
        }

        let invoice_lower = invoice_seller.to_lowercase();
        let payment_lower = payment_merchant.to_lowercase();

        if invoice_lower == payment_lower {
            return 1.0;
        }

        if invoice_lower.contains(&payment_lower) || payment_lower.contains(&invoice_lower) {
            return 0.9;
        }

        let similarity = self.levenshtein_similarity(&invoice_lower, &payment_lower);
        if similarity > 0.7 {
            return similarity;
        }

        self.keyword_matching_score(&invoice_lower, &payment_lower)
    }

    pub fn score_time(&self, invoice_date: &NaiveDate, payment_time: &str) -> f64 {
        let payment_date = self.parse_datetime(payment_time);

        let days_diff = match payment_date {
            Some(pd) => (*invoice_date - pd.date()).num_days().abs(),
            None => return 0.5,
        };

        if days_diff == 0 {
            1.0
        } else if days_diff <= 1 {
            0.9
        } else if days_diff <= 3 {
            0.7
        } else if days_diff <= 7 {
            0.5
        } else {
            0.0
        }
    }

    pub fn score_category(&self, invoice_category: &InvoiceCategory, payment_category: &str) -> f64 {
        let category_keywords = match invoice_category {
            InvoiceCategory::Hotel => vec!["酒店", "住宿", "宾馆", "旅馆", "饭店"],
            InvoiceCategory::CityTransport => vec!["滴滴", "高德", "交通", "出租", "网约车"],
            InvoiceCategory::Flight => vec!["航空", "机票", "航班", "飞机"],
            InvoiceCategory::Train => vec!["铁路", "高铁", "火车", "动车"],
            InvoiceCategory::Meal => vec!["餐饮", "饭店", "食品", "餐", "饭"],
            _ => return 0.5,
        };

        let payment_lower = payment_category.to_lowercase();
        if category_keywords.iter().any(|k| payment_lower.contains(k)) {
            1.0
        } else {
            0.0
        }
    }

    pub fn levenshtein_similarity(&self, s1: &str, s2: &str) -> f64 {
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();

        if len1 == 0 || len2 == 0 {
            return 0.0;
        }

        let distance = self.levenshtein_distance(s1, s2);
        1.0 - (distance as f64 / (len1.max(len2) as f64))
    }

    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        let chars1: Vec<char> = s1.chars().collect();
        let chars2: Vec<char> = s2.chars().collect();
        let len1 = chars1.len();
        let len2 = chars2.len();

        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for i in 0..=len1 {
            matrix[i][0] = i;
        }

        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if chars1[i - 1] == chars2[j - 1] { 0 } else { 1 };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[len1][len2]
    }

    pub fn keyword_matching_score(&self, seller: &str, merchant: &str) -> f64 {
        let hotel_brands = vec!["如家", "汉庭", "锦江", "7天", "华住", "希尔顿", "万豪", "喜来登", "香格里拉"];
        for brand in &hotel_brands {
            if seller.contains(brand) && merchant.contains(brand) {
                return 0.85;
            }
        }

        let ride_brands = vec!["滴滴", "高德", "t3", "曹操", "美团", "首汽"];
        for brand in &ride_brands {
            if seller.contains(brand) && merchant.contains(brand) {
                return 0.85;
            }
        }

        let food_brands = vec!["麦当劳", "肯德基", "星巴克", "必胜客", "真功夫", "永和大王"];
        for brand in &food_brands {
            if seller.contains(brand) && merchant.contains(brand) {
                return 0.85;
            }
        }

        0.0
    }

    fn parse_datetime(&self, time_str: &str) -> Option<NaiveDateTime> {
        let formats = vec![
            "%Y-%m-%d %H:%M",
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d",
            "%Y/%m/%d %H:%M",
            "%Y/%m/%d %H:%M:%S",
            "%Y/%m/%d",
        ];

        for fmt in formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(time_str, fmt) {
                return Some(dt);
            }
            if let Ok(d) = NaiveDate::parse_from_str(time_str, fmt) {
                return Some(d.and_hms_opt(0, 0, 0).unwrap());
            }
        }

        None
    }

    fn calculate_time_diff_hours(&self, invoice_date: &NaiveDate, payment_time: &str) -> f64 {
        let payment_date = match self.parse_datetime(payment_time) {
            Some(pd) => pd.date(),
            None => return 999.0,
        };

        let days_diff = (*invoice_date - payment_date).num_days().abs();
        days_diff as f64 * 24.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::InvoiceSource;
    use crate::models::payment::PaymentSource;
    use chrono::NaiveDate;

    fn make_invoice(id: &str, amount: f64, seller: &str, category: InvoiceCategory) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: format!("INV-{}", id),
            amount,
            seller_name: seller.to_string(),
            item_name: "Test Item".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            travel_date: None,
            category,
            source: InvoiceSource::Link("http://example.com".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
        }
    }

    fn make_payment(id: &str, amount: f64, merchant: &str, time: &str, category: &str) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            transaction_id: format!("TX-{}", id),
            transaction_time: time.to_string(),
            amount,
            original_amount: amount,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: merchant.to_string(),
            source: PaymentSource::Wechat,
            category: category.to_string(),
            payment_method: String::new(),
        }
    }

    #[test]
    fn test_score_amount_exact_match() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_amount(100.0, 100.0);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_score_amount_within_tolerance() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_amount(100.0, 103.0);
        assert!(score > 0.5 && score < 1.0);
    }

    #[test]
    fn test_score_amount_beyond_tolerance() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_amount(100.0, 110.0);
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_score_merchant_exact_match() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_merchant("如家酒店", "如家酒店");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_score_merchant_contains() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_merchant("北京如家酒店", "如家酒店");
        assert!(score >= 0.9);
    }

    #[test]
    fn test_score_merchant_keyword_match() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_merchant("如家酒店北京朝阳门店", "如家快捷酒店");
        assert!(score >= 0.85);
    }

    #[test]
    fn test_score_merchant_no_match() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_merchant("测试公司A", "测试公司B");
        assert!(score < 0.85);
    }

    #[test]
    fn test_score_time_same_day() {
        let scorer = MultiDimensionalScorer::default_weights();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let score = scorer.score_time(&date, "2025-01-15 10:30");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_score_time_one_day_diff() {
        let scorer = MultiDimensionalScorer::default_weights();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let score = scorer.score_time(&date, "2025-01-14 10:30");
        assert!((score - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_score_time_three_days_diff() {
        let scorer = MultiDimensionalScorer::default_weights();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let score = scorer.score_time(&date, "2025-01-12 10:30");
        assert!((score - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_score_time_week_diff() {
        let scorer = MultiDimensionalScorer::default_weights();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let score = scorer.score_time(&date, "2025-01-08 10:30");
        assert!((score - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_score_time_beyond_week() {
        let scorer = MultiDimensionalScorer::default_weights();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let score = scorer.score_time(&date, "2025-01-01 10:30");
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_score_category_match() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_category(&InvoiceCategory::Hotel, "住宿");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_score_category_no_match() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_category(&InvoiceCategory::Hotel, "交通");
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_levenshtein_similarity_identical() {
        let scorer = MultiDimensionalScorer::default_weights();
        let sim = scorer.levenshtein_similarity("测试", "测试");
        assert!((sim - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_levenshtein_similarity_similar() {
        let scorer = MultiDimensionalScorer::default_weights();
        let sim = scorer.levenshtein_similarity("北京如家酒店", "北京如家");
        assert!(sim > 0.5);
    }

    #[test]
    fn test_levenshtein_similarity_different() {
        let scorer = MultiDimensionalScorer::default_weights();
        let sim = scorer.levenshtein_similarity("测试A", "测试B");
        assert!(sim > 0.5);
    }

    #[test]
    fn test_full_score_calculation() {
        let scorer = MultiDimensionalScorer::default_weights();
        let invoice = make_invoice("inv1", 100.0, "如家酒店", InvoiceCategory::Hotel);
        let payment = make_payment("p1", 100.0, "如家快捷酒店", "2025-01-15 12:00", "住宿");

        let score = scorer.score(&invoice, &payment);

        assert!(score.total > 0.5);
        assert!((score.amount_score - 1.0).abs() < f64::EPSILON);
        assert!(score.merchant_score > 0.8);
        assert!((score.time_score - 1.0).abs() < f64::EPSILON);
        assert!((score.category_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_custom_weights() {
        let weights = ScoringWeights {
            amount: 0.6,
            merchant: 0.2,
            time: 0.1,
            category: 0.1,
        };
        let scorer = MultiDimensionalScorer::new(weights);

        let invoice = make_invoice("inv1", 100.0, "测试商家", InvoiceCategory::Other);
        let payment = make_payment("p1", 100.0, "测试商家", "2025-01-15 12:00", "其他");

        let score = scorer.score(&invoice, &payment);
        assert!(score.total > 0.8);
    }

    #[test]
    fn test_score_empty_strings() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_merchant("", "测试");
        assert!((score - 0.0).abs() < f64::EPSILON);

        let score = scorer.score_merchant("测试", "");
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_case_insensitive_merchant_match() {
        let scorer = MultiDimensionalScorer::default_weights();
        let score = scorer.score_merchant("STARBUCKS", "starbucks");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }
}
