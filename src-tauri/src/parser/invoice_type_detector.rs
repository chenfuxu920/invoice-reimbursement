use crate::ocr::structured_output::OcrStructuredOutput;
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
    Other,
}

pub struct InvoiceTypeDetector;

impl InvoiceTypeDetector {
    pub fn detect(ocr_output: &OcrStructuredOutput) -> InvoiceType {
        let all_text = ocr_output
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        if Self::is_ride_hailing_itinerary(&all_text) {
            return InvoiceType::RideHailingItinerary;
        }

        if Self::is_transit_card_statement(&all_text) {
            return InvoiceType::TransitCardStatement;
        }

        if Self::is_flight_invoice(&all_text) {
            return InvoiceType::FlightInvoice;
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
                && (text.contains("滴滴") || text.contains("高德") || text.contains("T3") || text.contains("曹操")))
    }

    fn is_transit_card_statement(text: &str) -> bool {
        text.contains("天府通")
            || text.contains("电子行程单") && (text.contains("公交") || text.contains("地铁"))
    }

    fn is_flight_invoice(text: &str) -> bool {
        text.contains("机票")
            || text.contains("航班")
            || text.contains("航空")
            || text.contains("行程单") && (text.contains("飞猪") || text.contains("携程") || text.contains("去哪儿"))
    }

    fn is_vat_electronic_invoice(text: &str) -> bool {
        text.contains("增值税") && text.contains("电子发票")
    }

    fn is_hotel_statement(text: &str) -> bool {
        (text.contains("结账单") || text.contains("账单")) && text.contains("酒店")
    }

    fn is_train_invoice(text: &str) -> bool {
        text.contains("火车票") || text.contains("铁路客票") || text.contains("高铁")
    }

    fn is_ride_hailing_invoice(text: &str) -> bool {
        (text.contains("滴滴") || text.contains("高德") || text.contains("T3") || text.contains("曹操"))
            && !text.contains("行程单")
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
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::VatElectronicInvoice);
    }

    #[test]
    fn test_detect_ride_hailing_itinerary_didi() {
        let ocr = create_ocr_output(vec!["滴滴出行行程单", "出发时间：2025-01-01"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::RideHailingItinerary);
    }

    #[test]
    fn test_detect_ride_hailing_itinerary_amap() {
        let ocr = create_ocr_output(vec!["高德打车行程单"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::RideHailingItinerary);
    }

    #[test]
    fn test_detect_transit_card_statement() {
        let ocr = create_ocr_output(vec!["天府通电子行程单", "地铁消费"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TransitCardStatement);
    }

    #[test]
    fn test_detect_flight_invoice() {
        let ocr = create_ocr_output(vec!["机票行程单", "航班号：CA1234"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::FlightInvoice);
    }

    #[test]
    fn test_detect_hotel_statement() {
        let ocr = create_ocr_output(vec!["酒店结账单", "住宿费"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::HotelStatement);
    }

    #[test]
    fn test_detect_train_invoice() {
        let ocr = create_ocr_output(vec!["火车票", "成都东站"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::TrainInvoice);
    }

    #[test]
    fn test_detect_ride_hailing_invoice() {
        let ocr = create_ocr_output(vec!["滴滴出行电子发票", "网约车服务"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::RideHailingInvoice);
    }

    #[test]
    fn test_detect_other() {
        let ocr = create_ocr_output(vec!["普通收据", "金额：50元"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::Other);
    }

    #[test]
    fn test_priority_itinerary_over_invoice() {
        let ocr = create_ocr_output(vec!["滴滴出行行程单"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::RideHailingItinerary);
    }

    #[test]
    fn test_flight_from_travel_platform() {
        let ocr = create_ocr_output(vec!["飞猪行程单", "航班信息"]);
        assert_eq!(InvoiceTypeDetector::detect(&ocr), InvoiceType::FlightInvoice);
    }
}
