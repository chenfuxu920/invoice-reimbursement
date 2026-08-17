//! Regression: 对照单 PDF 嵌入发票页时，源页可见区域必须保持。
//!
//! 两类问题：
//! 1. /Contents 数组流顺序颠倒 → 白色背景填充盖掉黑色正文（黑字消失）
//! 2. 移除源页 CropBox 后未显式裁剪 → CropBox 外的隐藏元素（如滴滴发票底部
//!    "didi" 水印）泄漏进导出页
//!
//! 本测试生成对照单 PDF，断言：
//!   - 输出内容流中白色背景填充位于正文文字之前（流顺序正确）
//!   - 内容流包含 `cm ... re W n`（矩阵后裁剪，按源 CropBox 保留可见区域）
//!   - q/Q 净深度为 0（变换矩阵不被多余 Q 弹掉）
//!   - 正文文字保留

use invoice_reimbursement_lib::models::invoice::{Invoice, InvoiceCategory, InvoiceSource};
use invoice_reimbursement_lib::models::match_result::{MatchResult, MatchType};
use invoice_reimbursement_lib::pdf::comparison_image_pdf_generator;
use lopdf::Object;
use std::path::Path;

const DZFP: &str = r"C:\Projects\rust-projects\invoice-reimbursement\data\住宿\13_【华住酒店集团】桔子成都省体育馆玉林路酒店发票已开具，感谢您_20260517_223242_dzfp_26512000002038107556_中国人民解放军国防科技大学系统工程学院_20260517223239.pdf";
const DIDI: &str =
    r"C:\Projects\rust-projects\invoice-reimbursement\data\市内交通\滴滴电子发票A.pdf";

fn make_match(pdf: &str, invoice_number: &str, category: InvoiceCategory) -> MatchResult {
    let invoice = Invoice {
        id: "inv1".to_string(),
        invoice_number: invoice_number.to_string(),
        amount: 100.0,
        seller_name: "test".to_string(),
        item_name: "item".to_string(),
        date: chrono::NaiveDate::from_ymd_opt(2026, 5, 17).unwrap(),
        travel_date: None,
        category,
        source: InvoiceSource::Pdf(pdf.to_string()),
        itineraries: vec![],
        itinerary_file: None,
        remarks: String::new(),
        hotel_detail: None,
        departure_city: None,
        arrival_city: None,
        toll_travel_time: None,
    };
    MatchResult {
        invoice_id: "inv1".to_string(),
        invoice,
        payment_ids: vec![],
        payments: vec![],
        match_type: MatchType::OneToOne,
        confidence: 1.0,
        amount_diff: 0.0,
        itinerary_payment_pairs: vec![],
        shared_payment_ids: vec![],
        shared_from_invoice_id: None,
    }
}

fn output_content_bytes(path: &str) -> Vec<u8> {
    let doc = lopdf::Document::load(path).unwrap();
    let pages = doc.get_pages();
    let page_id = *pages.get(&1).unwrap();
    let page = doc.get_object(page_id).unwrap();
    let dict = page.as_dict().unwrap();
    let contents = dict.get(b"Contents").unwrap().clone();
    let mut body = Vec::new();
    let mut stack = vec![contents];
    while let Some(o) = stack.pop() {
        match o {
            Object::Reference(id) => stack.push(doc.get_object(id).unwrap().clone()),
            Object::Stream(s) => {
                let mut s2 = s.clone();
                let _ = s2.decompress();
                body.extend(s2.content);
            }
            Object::Array(arr) => {
                for x in arr.iter().rev() {
                    stack.push(x.clone());
                }
            }
            _ => {}
        }
    }
    body
}

/// 跳过字符串字面量/十六进制串，统计内容流中 q/Q 净深度
fn q_net_depth(body: &[u8]) -> (i32, i32) {
    let mut depth = 0i32;
    let mut min_depth = 0i32;
    let mut i = 0;
    while i < body.len() {
        let c = body[i];
        match c {
            b'(' => {
                let mut d = 0;
                i += 1;
                while i < body.len() {
                    if body[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if body[i] == b'(' {
                        d += 1;
                    } else if body[i] == b')' {
                        if d == 0 {
                            break;
                        }
                        d -= 1;
                    }
                    i += 1;
                }
            }
            b'<' => {
                while i < body.len() && body[i] != b'>' {
                    i += 1;
                }
            }
            b' ' | b'\t' | b'\r' | b'\n' | b'\0' => {}
            b'q' | b'Q' => {
                let end = i + 1;
                if end < body.len() && !body[end].is_ascii_whitespace() {
                    // 更长 token 的一部分（如 "quads"），跳过
                } else if c == b'q' {
                    depth += 1;
                } else {
                    depth -= 1;
                    min_depth = min_depth.min(depth);
                }
            }
            _ => {}
        }
        i += 1;
    }
    (depth, min_depth)
}

#[test]
fn comparison_pdf_keeps_source_visible_area() {
    let out = std::env::temp_dir().join("regress_clip_didi.pdf");
    let out_s = out.to_string_lossy().to_string();
    if !Path::new(DIDI).exists() {
        eprintln!("SKIP: 测试 PDF 不存在: {DIDI}");
        return;
    }
    comparison_image_pdf_generator::generate_comparison_image_pdf(
        &[make_match(
            DIDI,
            "26517000000358455168",
            InvoiceCategory::CityTransport,
        )],
        "",
        &out_s,
        None,
    )
    .unwrap();

    let body = output_content_bytes(&out_s);
    let text = String::from_utf8_lossy(&body);

    // 1. 输出内容流必须是 `q → 矩阵 cm → 裁剪 re W n → 正文 …`
    //    clip 必须在 cm 之后（clip 与正文同坐标系，经矩阵映射到目标页）。
    let cm_pos = body.windows(2).position(|w| w == b"cm");
    let clip_pos = body.windows(7).position(|w| w == b"re W n\n");
    assert!(
        cm_pos.is_some(),
        "内容流应包含变换矩阵 cm:\n{}",
        &text[..text.len().min(300)]
    );
    assert!(
        clip_pos.is_some(),
        "内容流应包含裁剪 `re W n`（复刻源页可见区域）:\n{}",
        &text[..text.len().min(300)]
    );
    assert!(
        cm_pos.unwrap() < clip_pos.unwrap(),
        "裁剪必须在矩阵 cm 之后（cm 位于 {cm_pos:?}, clip 位于 {clip_pos:?}），否则裁剪坐标系错误导致整页被裁掉"
    );

    // 2. 整体 q/Q 平衡：净深度为 0
    let (net, min) = q_net_depth(&body);
    assert_eq!(net, 0, "内容流 q/Q 净深度应为 0，实际 {net}");
    assert!(min >= 0, "内容流 q/Q 最小深度不应为负，实际 {min}");

    // 3. 正文文字保留
    let words = invoice_reimbursement_lib::pdf::text_extractor::extract_raw_words_debug(&out_s)
        .unwrap_or_else(|e| panic!("输出 PDF 文字提取失败: {e}"));
    assert!(
        words.len() >= 30,
        "输出 PDF 应保留发票文字，实际只提取到 {} words",
        words.len()
    );
}

#[test]
fn comparison_pdf_keeps_contents_stream_order() {
    let out = std::env::temp_dir().join("regress_dzfp_order.pdf");
    let out_s = out.to_string_lossy().to_string();
    if !Path::new(DZFP).exists() {
        eprintln!("SKIP: 测试 PDF 不存在: {DZFP}");
        return;
    }
    comparison_image_pdf_generator::generate_comparison_image_pdf(
        &[make_match(
            DZFP,
            "26512000002038107556",
            InvoiceCategory::Hotel,
        )],
        "",
        &out_s,
        None,
    )
    .unwrap();

    let body = output_content_bytes(&out_s);
    let text = String::from_utf8_lossy(&body);

    // 白色填充在正文文字之前（否则文字被白底盖掉）
    let white_fill_pos = body.windows(8).position(|w| w == b"1 1 1  s");
    let text_start = body.windows(3).position(|w| w == b"BT\n");
    assert!(
        white_fill_pos.is_some(),
        "输出内容流应包含白色背景填充 `1 1 1 scn`，实际:\n{}",
        &text[..text.len().min(400)]
    );
    assert!(text_start.is_some(), "输出内容流应包含正文文字 `BT`");
    assert!(
        white_fill_pos.unwrap() < text_start.unwrap(),
        "白色背景填充 (pos {}) 必须在正文文字 (pos {}) 之前，否则黑色字体被白底盖掉",
        white_fill_pos.unwrap(),
        text_start.unwrap()
    );

    let (net, min) = q_net_depth(&body);
    assert_eq!(net, 0, "内容流 q/Q 净深度应为 0，实际 {net}");
    assert!(min >= 0, "内容流 q/Q 最小深度不应为负，实际 {min}");

    let words = invoice_reimbursement_lib::pdf::text_extractor::extract_raw_words_debug(&out_s)
        .unwrap_or_else(|e| panic!("输出 PDF 文字提取失败: {e}"));
    assert!(
        words.len() >= 50,
        "输出 PDF 应保留完整发票文字，实际只提取到 {} words",
        words.len()
    );
}
