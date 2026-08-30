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
    /// 实际可匹配的支付净值 = 实际支付金额（已扣除退款） + 优惠金额
    /// 注意：解析器输出的 amount 已扣除退款，这里不能再减一次 refund_amount
    pub fn total_value(&self) -> f64 {
        self.amount.max(0.0) + self.discount
    }

    /// 是否是退款/收入方向的记录（净支付金额 <= 0）。
    /// 部分退款的支付净额仍为正，不算退款记录，应按净额参与匹配。
    pub fn is_refund(&self) -> bool {
        self.amount <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_total_value_does_not_double_deduct_refund() {
        // 解析器输出的 amount 已是扣除退款后的净额（原 100 退 20 净 80）
        let mut p = PaymentRecord::default();
        p.amount = 80.0;
        p.original_amount = 100.0;
        p.refund_amount = 20.0;
        assert!((p.total_value() - 80.0).abs() < 1e-9);
    }

    #[test]
    fn test_partial_refund_is_not_a_refund_record() {
        let mut p = PaymentRecord::default();
        p.amount = 80.0;
        p.refund_amount = 20.0;
        assert!(!p.is_refund());

        p.amount = -30.0; // 收入/退款方向记录
        assert!(p.is_refund());
    }
}
