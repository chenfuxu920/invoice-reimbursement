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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub invoice_id: String,
    pub invoice: Invoice,
    pub payment_ids: Vec<String>,
    pub payments: Vec<PaymentRecord>,
    pub match_type: MatchType,
    pub confidence: f64,             // 匹配置信度 0-1
    pub amount_diff: f64,            // 金额差异
}
