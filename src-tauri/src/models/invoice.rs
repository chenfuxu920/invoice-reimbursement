use serde::{Deserialize, Serialize};
use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InvoiceCategory {
    Train,          // 高铁/车船票
    Flight,         // 飞机票
    Insurance,      // 保险费
    TicketChange,   // 退改签
    CityTransport,  // 市内交通
    Hotel,          // 住宿费
    Meal,           // 餐饮费
    Toll,           // 高速通行费
    Other,          // 其他
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "path")]
pub enum InvoiceSource {
    Photo(String),       // 照片路径
    Pdf(String),         // PDF路径
    Link(String),        // 发票链接
    Manual,              // 手动添加的空发票（无源文件，用于粘贴纸质票据）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub invoice_number: String,      // 发票号码
    pub amount: f64,                  // 金额
    pub seller_name: String,          // 销售方名称
    pub item_name: String,            // 项目名称
    pub date: NaiveDate,              // 开票日期
    pub travel_date: Option<NaiveDate>,  // 票面实际出行日期（仅 Train/Flight 类发票有值）
    pub category: InvoiceCategory,    // 自动识别的类别
    pub source: InvoiceSource,        // 来源
    pub itineraries: Vec<Itinerary>,  // 行程（打车场景）
    pub itinerary_file: Option<String>, // 关联的行程单文件（仅市内交通）
    #[serde(default)]
    pub remarks: String,              // 备注栏内容（前端手动创建的发票可能不含此字段，默认空串）
    pub hotel_detail: Option<HotelDetail>, // 住宿详情（仅住宿发票）
    // NEW: 票据出发/到达城市（仅 Train/Flight 类发票有值）
    pub departure_city: Option<String>,
    pub arrival_city: Option<String>,
    #[serde(default)]
    pub toll_travel_time: Option<chrono::NaiveDateTime>,  // 通行时间（从备注提取，仅 Toll 类）
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Itinerary {
    pub date_time: String,     // 行程时间
    pub provider: String,      // 服务商（滴滴/高德）
    pub pickup: String,        // 上车点
    pub dropoff: String,       // 下车点
    pub amount: f64,           // 行程金额
    /// 打车所在城市（行程单"城市"列；缺失时为空串）
    #[serde(default)]
    pub city: String,
    /// 缺失/未提取到的字段名列表，如 ["date_time"] 表示分钟数缺失
    #[serde(default)]
    pub incomplete_fields: Vec<String>,
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
        let manual = InvoiceSource::Manual;

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
        assert!(matches!(manual, InvoiceSource::Manual));
    }

    #[test]
    fn test_manual_source_serialize_no_path() {
        // Manual 是无数据的单元变体，序列化后不应包含 path 字段
        let src = InvoiceSource::Manual;
        let json = serde_json::to_string(&src).unwrap();
        assert_eq!(json, r#"{"type":"Manual"}"#);
        let de: InvoiceSource = serde_json::from_str(&json).unwrap();
        assert!(matches!(de, InvoiceSource::Manual));
    }

    #[test]
    fn test_deserialize_full_invoice_with_manual_source() {
        // 模拟前端 invoke('auto_match', { invoices }) 发送的 JSON：
        // 一张手动添加的空发票，source 为 {"type":"Manual"}（无 path 字段）
        let json = r#"{
            "id": "manual-blank-1",
            "invoice_number": "",
            "amount": 50.0,
            "seller_name": "",
            "item_name": "",
            "date": "2026-06-25",
            "travel_date": null,
            "category": "Meal",
            "source": {"type": "Manual"},
            "itineraries": [],
            "itinerary_file": null,
            "remarks": "",
            "hotel_detail": null,
            "departure_city": null,
            "arrival_city": null
        }"#;
        let inv: Invoice = serde_json::from_str(json).unwrap_or_else(|e| {
            panic!("反序列化 Manual 发票失败，这正是 auto_match 无响应的根因: {}", e)
        });
        assert!(matches!(inv.source, InvoiceSource::Manual));
        assert!((inv.amount - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deserialize_minimal_frontend_invoice() {
        // 模拟前端 BlankInvoiceEntryModal 实际发送的最小 JSON：
        // 缺少 remarks / itinerary_file / hotel_detail / departure_city / arrival_city
        // （前端 Invoice 类型不包含这些字段）
        let json = r#"{
            "id": "manual-blank-1",
            "invoice_number": "",
            "amount": 50.0,
            "seller_name": "",
            "item_name": "",
            "date": "2026-06-25",
            "category": "Meal",
            "source": {"type": "Manual"},
            "itineraries": []
        }"#;
        let result: Result<Invoice, _> = serde_json::from_str(json);
        if let Err(ref e) = result {
            eprintln!("最小前端 Invoice 反序列化失败: {}", e);
        }
        let inv = result.expect("前端最小 Invoice 应能反序列化（缺少的字段需 serde default）");
        assert!(matches!(inv.source, InvoiceSource::Manual));
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
            travel_date: None,
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Pdf("test.pdf".to_string()),
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: None,
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
        let itin = Itinerary { city: String::new(),
            date_time: "2025-06-15 10:30".to_string(),
            provider: "滴滴".to_string(),
            pickup: "北京站".to_string(),
            dropoff: "国贸".to_string(),
            amount: 35.0,
            incomplete_fields: vec![],
        };
        let invoice = Invoice {
            id: "taxi-1".to_string(),
            invoice_number: "TX001".to_string(),
            amount: 35.0,
            seller_name: "滴滴出行".to_string(),
            item_name: "网约车".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 6, 15).unwrap(),
            travel_date: None,
            category: InvoiceCategory::CityTransport,
            source: InvoiceSource::Photo("taxi.jpg".to_string()),
            itineraries: vec![itin.clone()],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: None,
        };
        assert_eq!(invoice.itineraries.len(), 1);
        assert_eq!(invoice.itineraries[0].provider, "滴滴");
        assert!((invoice.itineraries[0].amount - 35.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_invoice_category_serialize_deserialization() {
        let cat = InvoiceCategory::Hotel;
        let json = serde_json::to_string(&cat).unwrap();
        let deserialized: InvoiceCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(cat, deserialized);
    }

    #[test]
    fn test_toll_category_exists() {
        let toll = InvoiceCategory::Toll;
        assert_eq!(toll, InvoiceCategory::Toll);
        assert_ne!(toll, InvoiceCategory::Other);
        assert_ne!(toll, InvoiceCategory::CityTransport);
    }

    #[test]
    fn test_invoice_with_toll_travel_time() {
        let invoice = Invoice {
            id: "inv1".to_string(),
            invoice_number: String::new(),
            amount: 10.0,
            seller_name: String::new(),
            item_name: String::new(),
            date: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            travel_date: None,
            category: InvoiceCategory::Toll,
            source: InvoiceSource::Manual,
            itineraries: vec![],
            itinerary_file: None,
            remarks: String::new(),
            hotel_detail: None,
            departure_city: None,
            arrival_city: None,
            toll_travel_time: Some(
                chrono::NaiveDateTime::parse_from_str("2026-05-25 10:06:04", "%Y-%m-%d %H:%M:%S").unwrap()
            ),
        };
        assert!(invoice.toll_travel_time.is_some());
        assert_eq!(invoice.category, InvoiceCategory::Toll);
    }

    #[test]
    fn test_invoice_toll_travel_time_serde_default() {
        // 旧数据无 toll_travel_time 字段，反序列化应默认 None
        let json = r#"{
            "id":"inv1","invoice_number":"","amount":10.0,"seller_name":"",
            "item_name":"","date":"2026-05-25","travel_date":null,
            "category":"Toll","source":{"type":"Manual"},
            "itineraries":[],"itinerary_file":null,"remarks":"",
            "hotel_detail":null,"departure_city":null,"arrival_city":null
        }"#;
        let invoice: Invoice = serde_json::from_str(json).unwrap();
         assert!(invoice.toll_travel_time.is_none());
     }
}
