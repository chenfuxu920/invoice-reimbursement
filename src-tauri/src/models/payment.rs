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
    pub amount: f64,                 // 实际支付金额（已扣除退款）
    pub original_amount: f64,        // 原始金额（退款前）
    pub refund_amount: f64,          // 退款金额
    pub discount: f64,               // 优惠金额（平台补贴等）
    pub merchant_name: String,       // 商户名称
    pub source: PaymentSource,       // 来源
    pub category: String,            // 交易类型
    pub payment_method: String,      // 支付方式（信用卡/零钱/余额宝等）
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
    /// 商家实际收到的金额 = 支付金额 + 优惠金额
    /// 这个金额应该等于发票金额或行程单金额
    pub fn total_value(&self) -> f64 {
        self.amount + self.discount
    }
}
