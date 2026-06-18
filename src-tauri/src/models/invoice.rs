use serde::{Deserialize, Serialize};
use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[serde(tag = "type", content = "path")]
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
    pub itinerary_file: Option<String>, // 关联的行程单文件（仅市内交通）
    pub remarks: String,              // 备注栏内容
    pub hotel_detail: Option<HotelDetail>, // 住宿详情（仅住宿发票）
    // NEW: 票据出发/到达城市（仅 Train/Flight 类发票有值）
    pub departure_city: Option<String>,
    pub arrival_city: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Itinerary {
    pub date_time: String,     // 行程时间
    pub provider: String,      // 服务商（滴滴/高德）
    pub pickup: String,        // 上车点
    pub dropoff: String,       // 下车点
    pub amount: f64,           // 行程金额
}

/// 住宿发票详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotelDetail {
    pub check_in: Option<NaiveDate>,   // 入住日期
    pub check_out: Option<NaiveDate>,  // 离店日期
    pub nights: usize,                 // 住宿天数
    pub nightly_rate: f64,             // 实际每晚均价
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invoice_category_equality() {
        assert_eq!(InvoiceCategory::Train, InvoiceCategory::Train);
        assert_ne!(InvoiceCategory::Train, InvoiceCategory::Flight);
    }

    #[test]
    fn test_invoice_category_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(InvoiceCategory::Train);
        set.insert(InvoiceCategory::Train);
        set.insert(InvoiceCategory::Flight);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_invoice_source_variants() {
        let photo = InvoiceSource::Photo("img.jpg".to_string());
        let pdf = InvoiceSource::Pdf("doc.pdf".to_string());
        let link = InvoiceSource::Link("http://example.com".to_string());

        if let InvoiceSource::Photo(p) = photo {
            assert_eq!(p, "img.jpg");
        } else {
            panic!("Expected Photo variant");
        }
        if let InvoiceSource::Pdf(p) = pdf {
            assert_eq!(p, "doc.pdf");
        } else {
            panic!("Expected Pdf variant");
        }
        if let InvoiceSource::Link(p) = link {
            assert_eq!(p, "http://example.com");
        } else {
            panic!("Expected Link variant");
        }
    }

    #[test]
    fn test_invoice_construction() {
        let invoice = Invoice {
            id: "test-id".to_string(),
            invoice_number: "INV001".to_string(),
            amount: 123.45,
            seller_name: "测试公司".to_string(),
            item_name: "交通服务".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 6, 15).unwrap(),
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Pdf("test.pdf".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
        };
        assert_eq!(invoice.id, "test-id");
        assert_eq!(invoice.invoice_number, "INV001");
        assert!((invoice.amount - 123.45).abs() < f64::EPSILON);
        assert_eq!(invoice.seller_name, "测试公司");
        assert_eq!(invoice.category, InvoiceCategory::CityTransport);
        assert!(invoice.itineraries.is_empty());
    }

    #[test]
    fn test_invoice_with_itineraries() {
        let itin = Itinerary {
            date_time: "2025-06-15 10:30".to_string(),
            provider: "滴滴".to_string(),
            pickup: "北京站".to_string(),
            dropoff: "国贸".to_string(),
            amount: 35.0,
        };
        let invoice = Invoice {
            id: "taxi-1".to_string(),
            invoice_number: "TX001".to_string(),
            amount: 35.0,
            seller_name: "滴滴出行".to_string(),
            item_name: "网约车".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 6, 15).unwrap(),
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Photo("taxi.jpg".to_string()),
            itineraries: vec![itin.clone()],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
        };
        assert_eq!(invoice.itineraries.len(), 1);
        assert_eq!(invoice.itineraries[0].provider, "滴滴");
        assert!((invoice.itineraries[0].amount - 35.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_invoice_category_serialize_deserialize() {
        let cat = InvoiceCategory::Hotel;
        let json = serde_json::to_string(&cat).unwrap();
        let deserialized: InvoiceCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(cat, deserialized);
    }
}
