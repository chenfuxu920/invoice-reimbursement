use serde::{Deserialize, Serialize};
use super::invoice::Invoice;
use super::payment::PaymentRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    OneToOne,            // 1张发票 → 1笔支付
    OneToMany,           // 1张发票 → 多笔支付（打车）
    Unmatched,           // 未匹配
    ManualConfirmed,     // 手动确认
}

/// 行程条目与支付记录的显式配对关系。
/// 用于市内交通（网约车）等一对多场景：一张行程单发票含多条行程，
/// 每条行程对应一笔支付。此配对取代过去"按数组顺序隐式对应"的脆弱假设。
/// itinerary_index 对应 invoice.itineraries 的下标，payment_id 对应 payments 中的支付 id。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItineraryPaymentPair {
    pub itinerary_index: usize,
    pub payment_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub invoice_id: String,
    pub invoice: Invoice,
    pub payment_ids: Vec<String>,
    pub payments: Vec<PaymentRecord>,
    pub match_type: MatchType,
    pub confidence: f64,             // 匹配置信度 0-1
    pub amount_diff: f64,            // 金额差异
    /// 行程-支付显式配对。非行程场景或旧数据为空，导出层回退按 payments 索引对应。
    #[serde(default)]
    pub itinerary_payment_pairs: Vec<ItineraryPaymentPair>,
}

impl MatchResult {
    /// 按行程索引查找对应支付。
    /// 优先用 `itinerary_payment_pairs` 显式配对查找；无配对（旧数据或非行程场景）
    /// 时回退按 `payments` 数组索引对应，保持向后兼容。
    pub fn payment_for_itinerary(&self, itinerary_index: usize) -> Option<&PaymentRecord> {
        self.itinerary_payment_pairs
            .iter()
            .find(|p| p.itinerary_index == itinerary_index)
            .and_then(|pair| self.payments.iter().find(|p| p.id == pair.payment_id))
            .or_else(|| self.payments.get(itinerary_index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::invoice::{InvoiceCategory, InvoiceSource};
    use crate::models::payment::PaymentSource;
    use chrono::NaiveDate;

    fn make_invoice(id: &str) -> Invoice {
        Invoice {
            id: id.to_string(),
            invoice_number: String::new(),
            amount: 100.0,
            seller_name: String::new(),
            item_name: String::new(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            travel_date: None,
            category: InvoiceCategory::Other,
            source: InvoiceSource::Manual,
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
        }
    }

    fn make_payment(id: &str) -> PaymentRecord {
        PaymentRecord {
            id: id.to_string(),
            transaction_id: format!("TX-{}", id),
            transaction_time: "2025-01-01 12:00".to_string(),
            amount: 50.0,
            original_amount: 50.0,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: "M".to_string(),
            source: PaymentSource::Wechat,
            category: String::new(),
            payment_method: String::new(),
        }
    }

    fn make_result(payments: Vec<PaymentRecord>, pairs: Vec<ItineraryPaymentPair>) -> MatchResult {
        MatchResult {
            invoice_id: "inv1".to_string(),
            invoice: make_invoice("inv1"),
            payment_ids: payments.iter().map(|p| p.id.clone()).collect(),
            payments,
            match_type: MatchType::ManualConfirmed,
            confidence: 1.0,
            amount_diff: 0.0,
            itinerary_payment_pairs: pairs,
        }
    }

    #[test]
    fn test_payment_for_itinerary_uses_pairs_when_present() {
        // payments 顺序 [p1, p2]，但 pairs 显式指定 行程0→p2, 行程1→p1
        let result = make_result(
            vec![make_payment("p1"), make_payment("p2")],
            vec![
                ItineraryPaymentPair { itinerary_index: 0, payment_id: "p2".to_string() },
                ItineraryPaymentPair { itinerary_index: 1, payment_id: "p1".to_string() },
            ],
        );
        assert_eq!(result.payment_for_itinerary(0).unwrap().id, "p2");
        assert_eq!(result.payment_for_itinerary(1).unwrap().id, "p1");
    }

    #[test]
    fn test_payment_for_itinerary_falls_back_to_index_when_no_pairs() {
        // 无 pairs（旧数据），回退按索引
        let result = make_result(vec![make_payment("p1"), make_payment("p2")], vec![]);
        assert_eq!(result.payment_for_itinerary(0).unwrap().id, "p1");
        assert_eq!(result.payment_for_itinerary(1).unwrap().id, "p2");
    }

    #[test]
    fn test_payment_for_itinerary_returns_none_when_out_of_range() {
        let result = make_result(vec![make_payment("p1")], vec![]);
        assert!(result.payment_for_itinerary(5).is_none());
    }
}
