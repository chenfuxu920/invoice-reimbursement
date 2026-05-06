use serde::{Deserialize, Serialize};
use chrono::NaiveDate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvoiceCategory {
    Train,          // 高铁/车船票
    Flight,         // 飞机票
    TicketChange,   // 退改签/保险费
    CityTransport,  // 市内交通
    Hotel,          // 住宿费
    Meal,           // 餐饮费
    Other,          // 其他
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvoiceSource {
    Photo(String),       // 照片路径
    Pdf(String),         // PDF路径
    Link(String),        // 发票链接
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub invoice_number: String,      // 发票号码
    pub amount: f64,                  // 金额
    pub seller_name: String,          // 销售方名称
    pub item_name: String,            // 项目名称
    pub date: NaiveDate,              // 开票日期
    pub category: InvoiceCategory,    // 自动识别的类别
    pub source: InvoiceSource,        // 来源
    pub itineraries: Vec<Itinerary>,  // 行程（打车场景）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Itinerary {
    pub date_time: String,     // 行程时间
    pub provider: String,      // 服务商（滴滴/高德）
    pub pickup: String,        // 上车点
    pub dropoff: String,       // 下车点
    pub amount: f64,           // 行程金额
}
