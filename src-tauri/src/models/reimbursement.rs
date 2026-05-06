use serde::{Deserialize, Serialize};
use super::invoice::InvoiceCategory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: InvoiceCategory,
    pub count: usize,                // 单据张数
    pub total_amount: f64,           // 申报金额
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReimbursementForm {
    pub name: String,                // 姓名
    pub department: String,          // 部职别
    pub travel_start: String,        // 出差开始日期
    pub travel_end: String,          // 出差结束日期
    pub companions: usize,           // 同行人数
    pub summaries: Vec<CategorySummary>,
    pub total_amount: f64,           // 总计
}
