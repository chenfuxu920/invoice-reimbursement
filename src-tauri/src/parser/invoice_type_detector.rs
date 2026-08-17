use crate::ocr::structured_output::OcrStructuredOutput;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvoiceType {
    VatElectronicInvoice,
    RideHailingInvoice,
    RideHailingItinerary,
    FlightInvoice,
    TrainInvoice,
    HotelStatement,
    TransitCardStatement,
    TollInvoice, // 高速通行费发票
    Other,
}

pub struct InvoiceTypeDetector;

impl InvoiceTypeDetector {
    /// 归一化 CJK 部首变体为标准汉字
    /// 修复华住酒店 PDF 中"电⼦发票"使用 U+2F26（CJK 部首⼦）而非标准 U+5B50（子）的问题
    fn normalize_cjk_radicals(text: &str) -> String {
        text.replace('\u{2F26}', "\u{5B50}") // ⼦ → 子
    }

    pub fn detect(ocr_output: &OcrStructuredOutput) -> InvoiceType {
        let raw_text = ocr_output
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let all_text = Self::normalize_cjk_radicals(&raw_text);

        if Self::is_ride_hailing_itinerary(&all_text) {
            return InvoiceType::RideHailingItinerary;
        }

        if Self::is_transit_card_statement(&all_text) {
            return InvoiceType::TransitCardStatement;
        }

        if Self::is_flight_invoice(&all_text) {
            return InvoiceType::FlightInvoice;
        }

        if Self::is_toll_invoice(&all_text) {
            return InvoiceType::TollInvoice;
        }

        if Self::is_vat_electronic_invoice(&all_text) {
            return InvoiceType::VatElectronicInvoice;
        }

        if Self::is_hotel_statement(&all_text) {
            return InvoiceType::HotelStatement;
        }

        if Self::is_train_invoice(&all_text) {
            return InvoiceType::TrainInvoice;
        }

        if Self::is_ride_hailing_invoice(&all_text) {
            return InvoiceType::RideHailingInvoice;
        }

        InvoiceType::Other
    }

    fn is_ride_hailing_itinerary(text: &str) -> bool {
        text.contains("行程报销单")
            || (text.contains("行程单")
                && (text.contains("滴滴")
                    || text.contains("高德")
                    || text.contains("T3")
                    || text.contains("曹操")))
    }

    fn is_transit_card_statement(text: &str) -> bool {
        text.contains("天府通")
            || text.contains("电子行程单") && (text.contains("公交") || text.contains("地铁"))
    }

    fn is_flight_invoice(text: &str) -> bool {
        // 排除保险费发票：如"国内机票航空意外险"含"机票"+"航空"但实为保险
        if text.contains("保险") || text.contains("意外险") {
            return false;
        }
        text.contains("机票")
            || text.contains("航班")
            || text.contains("航空")
            || text.contains("行程单")
                && (text.contains("飞猪") || text.contains("携程") || text.contains("去哪儿"))
    }

    fn is_vat_electronic_invoice(text: &str) -> bool {
        // 增值税电子发票（原逻辑）
        if text.contains("增值税") && text.contains("电子发票") {
            return true;
        }
        // 电子普通发票（如众安保险、华住酒店），但排除网约车发票
        // 滴滴/高德等电子发票也是"电子发票（普通发票）"格式，应归 RideHailingInvoice
        text.contains("电子发票")
            && text.contains("普通发票")
            && !Self::is_ride_hailing_invoice(text)
    }

    fn is_hotel_statement(text: &str) -> bool {
        (text.contains("结账单") || text.contains("账单")) && text.contains("酒店")
    }

    fn is_train_invoice(text: &str) -> bool {
        text.contains("火车票")
            || text.contains("铁路客票")
            || text.contains("铁路电子客票")
            || text.contains("高铁")
            || Regex::new(r"[GD]\d+").unwrap().is_match(text)
                && (text.contains("站") || text.contains("铁路"))
    }

    fn is_ride_hailing_invoice(text: &str) -> bool {
        (text.contains("滴滴")
            || text.contains("高德")
            || text.contains("T3")
            || text.contains("曹操"))
            && !text.contains("行程单")
    }

    fn is_toll_invoice(text: &str) -> bool {
        text.contains("通行费")
            || text.contains("过路费")
            || (text.contains("ETC") && text.contains("高速"))
            || (text.contains("高速") && text.contains("电子发票"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::structured_output::{BoundingBox, OcrTextBlock, PageLayout, TextBlockType};

    fn create_ocr_output(texts: Vec<&str>) -> OcrStructuredOutput {
        let blocks = texts
            .iter()
            .enumerate()
            .map(|(i, text)| OcrTextBlock {
                text: text.to_string(),
                confidence: 0.95,
                bbox: BoundingBox::default(),
                line_index: i,
                block_type: TextBlockType::Other,
            })
            .collect();

        OcrStructuredOutput {
            blocks,
            layout: PageLayout::default(),
        }
    }

    #[test]
    fn test_detect_vat_electronic_invoice() {
        let ocr = create_ocr_output(vec!["增值税电子发票", "价税合计：100.00"]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::VatElectronicInvoice
        );
    }

    #[test]
    fn test_detect_ride_hailing_itinerary_didi() {
        let ocr = create_ocr_output(vec!["滴滴出行行程单", "出发时间：2025-01-01"]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::RideHailingItinerary
        );
    }

    #[test]
    fn test_detect_ride_hailing_itinerary_amap() {
        let ocr = create_ocr_output(vec!["高德打车行程单"]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::RideHailingItinerary
        );
    }

    #[test]
    fn test_detect_transit_card_statement() {
        let ocr = create_ocr_output(vec!["天府通电子行程单", "地铁消费"]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::TransitCardStatement
        );
    }

    #[test]
    fn test_detect_flight_invoice() {
        let ocr = create_ocr_output(vec!["机票行程单", "航班号：CA1234"]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::FlightInvoice
        );
    }

    #[test]
    fn test_detect_hotel_statement() {
        let ocr = create_ocr_output(vec!["酒店结账单", "住宿费"]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::HotelStatement
        );
    }

    #[test]
    fn test_detect_train_invoice() {
        let ocr = create_ocr_output(vec!["火车票", "成都东站"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TrainInvoice);
    }

    #[test]
    fn test_detect_railway_electronic_ticket() {
        let ocr = create_ocr_output(vec![
            "电子发票（铁路电子客票）",
            "G878",
            "长沙南站",
            "武汉站",
        ]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TrainInvoice);
    }

    #[test]
    fn test_detect_high_speed_train_number() {
        let ocr = create_ocr_output(vec!["D1234", "北京站", "上海站"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TrainInvoice);
    }

    #[test]
    fn test_detect_ride_hailing_invoice() {
        let ocr = create_ocr_output(vec!["滴滴出行电子发票", "网约车服务"]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::RideHailingInvoice
        );
    }

    #[test]
    fn test_detect_other() {
        let ocr = create_ocr_output(vec!["普通收据", "金额：50元"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::Other);
    }

    #[test]
    fn test_priority_itinerary_over_invoice() {
        let ocr = create_ocr_output(vec!["滴滴出行行程单"]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::RideHailingItinerary
        );
    }

    #[test]
    fn test_flight_from_travel_platform() {
        let ocr = create_ocr_output(vec!["飞猪行程单", "航班信息"]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::FlightInvoice
        );
    }

    #[test]
    fn test_detect_insurance_invoice_not_flight() {
        // 机票保险费发票含"机票"+"航空"+"保险"，不应识别为 FlightInvoice
        // 真实样本：众安在线财产保险 *保险服务*国内机票航空意外险
        let ocr = create_ocr_output(vec![
            "电子发票（普通发票）",
            "*保险服务*国内机票航空意外险",
            "众安在线财产保险股份有限公司",
            "价税合计：¥50.00",
        ]);
        assert_ne!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::FlightInvoice
        );
    }

    #[test]
    fn test_detect_toll_invoice_by_keyword() {
        let ocr = create_ocr_output(vec!["通行费", "增值税电子发票", "价税合计：10.00"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TollInvoice);
    }

    #[test]
    fn test_detect_toll_invoice_etc() {
        let ocr = create_ocr_output(vec!["ETC通行费", "高速公路", "金额：15.50"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TollInvoice);
    }

    #[test]
    fn test_detect_toll_invoice_overpass_fee() {
        let ocr = create_ocr_output(vec!["过路费", "电子发票"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TollInvoice);
    }

    #[test]
    fn test_detect_toll_priority_over_vat_electronic() {
        // 同时含"增值税电子发票"和"通行费"，应优先识别为 Toll
        let ocr = create_ocr_output(vec!["增值税电子发票", "通行费", "价税合计：10.00"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TollInvoice);
    }

    #[test]
    fn test_detect_general_electronic_invoice() {
        // 电子普通发票（不含"增值税"）应识别为 VatElectronicInvoice
        // 真实样本：众安保险"电子发票（普通发票）"
        let ocr = create_ocr_output(vec![
            "电子发票（普通发票）",
            "*保险服务*国内机票航空意外险",
            "众安在线财产保险股份有限公司",
            "价税合计：¥50.00",
        ]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::VatElectronicInvoice
        );
    }

    #[test]
    fn test_detect_general_electronic_not_ride_hailing() {
        // 滴滴电子发票也是"电子发票（普通发票）"格式，但应归 RideHailingInvoice
        // 防止方案A放宽后引入回归
        let ocr = create_ocr_output(vec![
            "电子发票（普通发票）",
            "发票号码: 26437000000202866011",
            "旅客运输服务",
            "湖南滴滴出行科技有限公司",
        ]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::RideHailingInvoice
        );
    }

    #[test]
    fn test_detect_cjk_radical_normalization() {
        // 华住酒店 PDF 中"电⼦发票"使用 U+2F26（CJK 部首⼦）而非标准 U+5B50（子）
        // 归一化后应能识别为 VatElectronicInvoice
        let ocr = create_ocr_output(vec![
            "电\u{2F26}发票（普通发票）", // ⼦ = U+2F26
            "*住宿服务*住宿费",
            "四川景澜酒店管理有限公司",
            "价税合计：¥2528.05",
        ]);
        assert_eq!(
            InvoiceTypeDetector::detect(&ocr),
            InvoiceType::VatElectronicInvoice
        );
    }
}
