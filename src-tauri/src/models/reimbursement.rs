use super::invoice::InvoiceCategory;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: InvoiceCategory,
    pub count: usize,      // 单据张数
    pub total_amount: f64, // 申报金额
}

/// 住宿费分级明细
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotelLevelDetail {
    pub level: String,      // 级别名称 (战区级以上/军级/师级/其他人员)
    pub persons: usize,     // 人数
    pub days: usize,        // 天数（住宿晚数）
    pub daily_rate: f64,    // 每日标准
    pub amount: f64,        // 可报销金额（封顶后）
    pub actual_amount: f64, // 实际支付金额
}

/// 伙食补助明细
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealSubsidyDetail {
    pub persons: usize,  // 人数
    pub days: usize,     // 天数
    pub daily_rate: f64, // 每日标准
    pub amount: f64,     // 申报金额
}

/// 城市间交通费明细
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportDetail {
    pub label: String, // 类别名 (车船票/飞机票/订退改签票)
    pub count: usize,  // 单据张数
    pub amount: f64,   // 申报金额
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReimbursementForm {
    pub name: String,         // 姓名
    pub department: String,   // 部职别
    pub destination: String,  // 到达地点
    pub travel_start: String, // 出差开始日期
    pub travel_end: String,   // 出差结束日期
    pub travel_days: usize,   // 出差天数
    pub companions: usize,    // 同行人数

    // 城市间交通费
    pub transport_details: Vec<TransportDetail>,
    pub transport_subtotal: f64,

    // 市内交通费
    pub city_transport_count: usize,
    pub city_transport_amount: f64,        // 可报销金额（封顶后）
    pub city_transport_actual_amount: f64, // 实际支出金额
    pub city_transport_daily_std: f64,     // 每日标准（元/天），供前端展示与超标分析

    // 住宿费
    pub hotel_levels: Vec<HotelLevelDetail>,
    pub hotel_subtotal: f64,

    // 伙食补助
    pub meal_subsidy: MealSubsidyDetail,

    // 其他
    pub baggage_amount: f64,     // 行李托运费
    pub meal_reimbursement: f64, // 凭据报销伙食费

    // 兼容旧接口
    pub summaries: Vec<CategorySummary>,
    pub total_amount: f64, // 申报金额合计
}
