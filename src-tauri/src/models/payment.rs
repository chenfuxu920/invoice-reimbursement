use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentSource {
    Wechat, // 微信
    Alipay, // 支付宝
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRecord {
    pub id: String,
    pub transaction_id: String,   // 交易单号
    pub transaction_time: String, // 交易时间
    pub amount: f64,              // 实际支付金额（已扣除退款）
    pub original_amount: f64,     // 原始金额（退款前）
    pub refund_amount: f64,       // 退款金额
    pub discount: f64,            // 优惠金额（平台补贴等）
    pub merchant_name: String,    // 商户名称
    pub source: PaymentSource,    // 来源
    pub category: String,         // 交易类型
    pub payment_method: String,   // 支付方式（信用卡/零钱/余额宝等）
}

impl Default for PaymentRecord {
    fn default() -> Self {
        Self {
            id: String::new(),
            transaction_id: String::new(),
            transaction_time: String::new(),
            amount: 0.0,
            original_amount: 0.0,
            refund_amount: 0.0,
            discount: 0.0,
            merchant_name: String::new(),
            source: PaymentSource::Wechat,
            category: String::new(),
            payment_method: String::new(),
        }
    }
}

impl PaymentRecord {
    /// 商家实际收到的金额 = max(支付金额 - 退款金额, 0) + 优惠金额
    /// 考虑退款后，实际可匹配的支付净值
    pub fn total_value(&self) -> f64 {
        (self.amount - self.refund_amount).max(0.0) + self.discount
    }

    /// 是否是退款交易（退款金额 > 0 或实际支付金额 <= 0）
    pub fn is_refund(&self) -> bool {
        self.refund_amount > 0.0 || self.amount <= 0.0
    }
}
