use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentSource {
    Wechat,     // 微信
    Alipay,     // 支付宝
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRecord {
    pub id: String,
    pub transaction_id: String,      // 交易单号
    pub transaction_time: String,    // 交易时间
    pub amount: f64,                 // 交易金额
    pub merchant_name: String,       // 商户名称
    pub source: PaymentSource,       // 来源
    pub category: String,            // 交易类型
}
