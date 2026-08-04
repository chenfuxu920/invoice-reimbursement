//! Regression: 对照单 PDF 嵌入发票页时，源页注解（如 Square annotation 外框）
//! 必须随页面矩阵一起变换。
//!
//! 背景：`place_page` 只把 `cm` 矩阵包裹到 `/Contents` 上，annotation 外观流
//! 不经过页面 CTM，按原始 /Rect 绘制。保险发票外框是 Square annotation，
//! 若 /Rect 不变换，外框会错位在导出页角落（内容缩放居中、边框留在原坐标）。
//!
//! 本测试用真实保险发票生成对照单 PDF，断言：
//!   - 输出页 /Annots 的 /Rect 已按矩阵映射（左边界从源坐标放大+平移）
//!   - /Rect 顺序为 [x0 y0 x1 y1]（x0<y0 段不混序）
//!   - 输出内容流包含 `cm` 变换矩阵

use invoice_reimbursement_lib::models::invoice::{Invoice, InvoiceCategory, InvoiceSource};
use invoice_reimbursement_lib::models::match_result::{MatchResult, MatchType};
use invoice_reimbursement_lib::pdf::comparison_image_pdf_generator;
use lopdf::Object;
use std::path::Path;

fn find_insurance_pdf() -> Option<String> {
    let data_root = r"C:\Projects\rust-projects\invoice-reimbursement\data";
    let mut stack = vec![Path::new(data_root).to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&d) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if name.contains("20260604_225419") && name.ends_with(".pdf") {
                        return Some(p.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    None
}

fn make_match(pdf: &str) -> MatchResult {
    let invoice = Invoice {
        id: "inv1".to_string(),
        invoice_number: "123".to_string(),
        amount: 20.0,
        seller_name: "test".to_string(),
        item_name: "item".to_string(),
        date: chrono::NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
        travel_date: None,
        category: InvoiceCategory::Insurance,
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

fn output_annotation_rects(path: &str) -> Vec<[f32; 4]> {
    let doc = lopdf::Document::load(path).unwrap();
    let page_id = *doc.get_pages().get(&1).unwrap();
    let page = doc.get_object(page_id).unwrap();
    let dict = page.as_dict().unwrap();
    let mut rects = Vec::new();
    if let Ok(annots) = dict.get(b"Annots") {
        let mut stack = vec![annots.clone()];
        while let Some(o) = stack.pop() {
            match o {
                Object::Array(arr) => {
                    for x in arr.iter().rev() {
                        stack.push(x.clone());
                    }
                }
                Object::Reference(rid) => {
                    if let Ok(obj) = doc.get_object(rid) {
                        if let Ok(ad) = obj.as_dict() {
                            if let Ok(Object::Array(corners)) = ad.get(b"Rect") {
                                if corners.len() == 4 {
                                    let mut r = [0f32; 4];
                                    for (i, c) in corners.iter().enumerate() {
                                        r[i] = c.as_float().unwrap_or(0.0);
                                    }
                                    rects.push(r);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    rects
}

#[test]
fn comparison_pdf_transforms_annotation_rect() {
    let pdf = match find_insurance_pdf() {
        Some(p) => p,
        None => {
            eprintln!("SKIP: 测试 PDF 不存在 (data/保险/20_电子发票_20260604_225419)");
            return;
        }
    };

    // 源页注解 Rect（真实发票外框）
    let src_doc = lopdf::Document::load(&pdf).unwrap();
    let src_page_id = *src_doc.get_pages().get(&1).unwrap();
    let src_page = src_doc.get_object(src_page_id).unwrap();
    let src_dict = src_page.as_dict().unwrap();
    let src_rect: [f32; 4] = {
        let annots = src_dict.get(b"Annots").unwrap();
        let mut r = None;
        let mut stack = vec![annots.clone()];
        while let Some(o) = stack.pop() {
            match o {
                Object::Array(arr) => {
                    for x in arr.iter().rev() {
                        stack.push(x.clone());
                    }
                }
                Object::Reference(rid) => {
                    if let Ok(obj) = src_doc.get_object(rid) {
                        if let Ok(ad) = obj.as_dict() {
                            if let Ok(Object::Array(corners)) = ad.get(b"Rect") {
                                if corners.len() == 4 {
                                    r = Some([
                                        corners[0].as_float().unwrap_or(0.0),
                                        corners[1].as_float().unwrap_or(0.0),
                                        corners[2].as_float().unwrap_or(0.0),
                                        corners[3].as_float().unwrap_or(0.0),
                                    ]);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        r.expect("源页应有 annotation Rect")
    };

    let out = std::env::temp_dir().join("regress_annotation_rect.pdf");
    let out_s = out.to_string_lossy().to_string();
    comparison_image_pdf_generator::generate_comparison_image_pdf(
        &[make_match(&pdf)],
        "",
        &out_s,
        None,
    )
    .unwrap();

    let rects = output_annotation_rects(&out_s);
    assert_eq!(rects.len(), 1, "输出页应保留 1 个 annotation");

    let r = rects[0];
    // 非旋转矩阵：s=min(avail_w/bw, avail_h/bh)，e/f 为平移
    let bw = src_rect[2] - src_rect[0];
    let bh = src_rect[3] - src_rect[1];
    let (src_bw, src_bh) = (595.276f32, 422.30807f32);
    let mm = 72.0 / 25.4;
    let avail_w = 842.0 - 2.0 * 8.0 * mm;
    let avail_h = 595.0 - 2.0 * 8.0 * mm - 20.0 * mm;
    let s = (avail_w / src_bw).min(avail_h / src_bh);
    let target_x = (842.0 - s * src_bw) / 2.0;
    let target_y = (595.0 - s * src_bh) / 2.0 + 5.0 * mm;

    // 期望映射：x' = s*x + (target_x - s*0) = s*x + target_x
    let expect_x0 = s * src_rect[0] + target_x;
    let expect_x1 = s * src_rect[2] + target_x;
    let expect_y0 = s * src_rect[1] + target_y;
    let expect_y1 = s * src_rect[3] + target_y;

    // 顺序必须是 [x0 y0 x1 y1]
    assert!(
        r[0] < r[2] && r[1] < r[3],
        "Rect 应保持 [x0 y0 x1 y1] 顺序，实际 {r:?}"
    );
    assert!(
        (r[0] - expect_x0).abs() < 1.0
            && (r[1] - expect_y0).abs() < 1.0
            && (r[2] - expect_x1).abs() < 1.0
            && (r[3] - expect_y1).abs() < 1.0,
        "annotation Rect 应按矩阵映射：期望 [{expect_x0} {expect_y0} {expect_x1} {expect_y1}]，实际 {r:?}"
    );

    // 内容流仍包含变换矩阵
    let doc = lopdf::Document::load(&out_s).unwrap();
    let page_id = *doc.get_pages().get(&1).unwrap();
    let page = doc.get_object(page_id).unwrap();
    let dict = page.as_dict().unwrap();
    let contents = dict.get(b"Contents").unwrap().clone();
    let mut body = Vec::new();
    let mut stack = vec![contents];
    while let Some(o) = stack.pop() {
        match o {
            Object::Reference(rid) => stack.push(doc.get_object(rid).unwrap().clone()),
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
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("cm") && text.contains("re W n"),
        "内容流应包含 cm 矩阵与裁剪:\n{}",
        &text[..text.len().min(200)]
    );
}
