use std::collections::BTreeSet;
use std::error::Error;
use std::path::Path;

use lopdf::{Object, ObjectId, Stream};
use medpdf::{create_blank_page, insert_content_stream};

use crate::models::hotel_standard::get_hotel_nightly_rate_std;
use crate::models::invoice::{InvoiceCategory, InvoiceSource};
use crate::models::match_result::MatchResult;
use super::cjk_font;

const PAGE_W: f32 = 842.0; // A4 横版 pt (297mm)
const PAGE_H: f32 = 595.0; // A4 横版 pt (210mm)
const MM: f32 = 72.0 / 25.4;

enum PageSpec {
    /// 原始 PDF 页矢量嵌入（发票 / 行程单页）
    Invoice {
        pdf: String,
        page: u32, // 0-based 页码
        rotate: bool,
        payment: String,
        over_std: Option<String>,
    },
    /// 手动空发票：虚线框 + 提示
    Blank {
        payment: String,
    },
    /// 市内交通行程表格
    Table {
        rows: Vec<(usize, f64, String)>,
    },
}

fn build_payment_text(result: &MatchResult) -> String {
    let has_table = matches!(result.invoice.category, InvoiceCategory::CityTransport)
        && !result.invoice.itineraries.is_empty();
    if has_table || result.payments.is_empty() {
        return String::new();
    }
    fn prefix_for(src: &crate::models::payment::PaymentSource) -> &'static str {
        match src {
            crate::models::payment::PaymentSource::Wechat => "微信单号：",
            crate::models::payment::PaymentSource::Alipay => "支付宝单号：",
        }
    }
    if result.payments.len() == 1 {
        let p = &result.payments[0];
        format!("{}{}", prefix_for(&p.source), p.transaction_id)
    } else {
        result
            .payments
            .iter()
            .map(|p| format!("{}{}", prefix_for(&p.source), p.transaction_id))
            .collect::<Vec<_>>()
            .join("，")
    }
}

fn build_over_std(result: &MatchResult, destination: Option<&str>) -> Option<String> {
    if !matches!(result.invoice.category, InvoiceCategory::Hotel) {
        return None;
    }
    let destination = destination?;
    let std = get_hotel_nightly_rate_std(destination);
    let detail = result.invoice.hotel_detail.as_ref()?;
    let standard_amount = std * detail.nights as f64;
    if result.invoice.amount > standard_amount + 0.01 {
        Some(format!(
            "发票金额{:.2}，实报{:.2}",
            result.invoice.amount, standard_amount
        ))
    } else {
        None
    }
}

fn new_document() -> lopdf::Document {
    let mut doc = lopdf::Document::with_version("1.5");
    let pages_id = doc.add_object(Object::Dictionary(lopdf::dictionary! {
        "Type" => "Pages",
        "Kids" => Object::Array(vec![]),
        "Count" => Object::Integer(0)
    }));
    let catalog_id = doc.add_object(Object::Dictionary(lopdf::dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id)
    }));
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc
}

/// 深拷贝一个对象图到目标文档（循环引用安全：先占位再递归）。
fn deep_copy_value(
    doc: &mut lopdf::Document,
    src: &lopdf::Document,
    obj: &Object,
    cache: &mut std::collections::BTreeMap<ObjectId, ObjectId>,
) -> Result<Object, Box<dyn Error>> {
    match obj {
        Object::Dictionary(d) => {
            let mut nd = lopdf::Dictionary::new();
            for (k, v) in d {
                if k == b"Parent" {
                    continue; // 不向上拷贝整棵页面树
                }
                if let Object::Reference(id) = v {
                    nd.set(k.clone(), Object::Reference(deep_copy(doc, src, *id, cache)?));
                } else {
                    nd.set(k.clone(), deep_copy_value(doc, src, v, cache)?);
                }
            }
            Ok(Object::Dictionary(nd))
        }
        Object::Array(a) => {
            let mut na = Vec::with_capacity(a.len());
            for v in a {
                if let Object::Reference(id) = v {
                    na.push(Object::Reference(deep_copy(doc, src, *id, cache)?));
                } else {
                    na.push(deep_copy_value(doc, src, v, cache)?);
                }
            }
            Ok(Object::Array(na))
        }
        Object::Stream(s) => {
            // 流对象字典里的引用必须重编号（如图像 /SMask、Form /Group），否则指向源文档对象 → 渲染黑块
            let mut new_dict = lopdf::Dictionary::new();
            for (k, v) in s.dict.iter() {
                if let Object::Reference(id) = v {
                    new_dict.set(k.clone(), Object::Reference(deep_copy(doc, src, *id, cache)?));
                } else {
                    new_dict.set(k.clone(), deep_copy_value(doc, src, v, cache)?);
                }
            }
            Ok(Object::Stream(Stream::new(new_dict, s.content.clone())))
        }
        other => Ok(other.clone()),
    }
}

fn deep_copy(
    doc: &mut lopdf::Document,
    src: &lopdf::Document,
    id: ObjectId,
    cache: &mut std::collections::BTreeMap<ObjectId, ObjectId>,
) -> Result<ObjectId, Box<dyn Error>> {
    if let Some(&new_id) = cache.get(&id) {
        return Ok(new_id);
    }
    // 先占位（Null），防止 A→B→A 循环递归爆栈
    let placeholder = doc.add_object(Object::Null);
    cache.insert(id, placeholder);
    let obj = src.get_object(id)?;
    let new_obj = deep_copy_value(doc, src, obj, cache)?;
    doc.objects.insert(placeholder, new_obj);
    Ok(placeholder)
}

/// 从源文档拷贝一页（0-based 页码）到目标文档，挂到目标 Pages 树。
fn copy_page_safe(
    doc: &mut lopdf::Document,
    src: &lopdf::Document,
    page_0based: u32,
) -> Result<ObjectId, Box<dyn Error>> {
    let src_page_id = *src
        .get_pages()
        .get(&(page_0based + 1))
        .ok_or("源 PDF 页面不存在")?;
    let mut cache = std::collections::BTreeMap::new();
    let new_page_id = deep_copy(doc, src, src_page_id, &mut cache)?;

    let pages_id = {
        let root_id = doc.trailer.get(b"Root")?.as_reference()?;
        let cat = doc.get_object_mut(root_id)?;
        cat.as_dict()?.get(b"Pages")?.as_reference()?
    };
    let page = doc.get_object_mut(new_page_id)?.as_dict_mut()?;
    page.set(b"Parent", Object::Reference(pages_id));
    let pages = doc.get_object_mut(pages_id)?.as_dict_mut()?;
    if let Ok(kids) = pages.get_mut(b"Kids") {
        if let Object::Array(arr) = kids {
            arr.push(Object::Reference(new_page_id));
        }
    }
    if let Ok(count) = pages.get_mut(b"Count") {
        if let Object::Integer(n) = count {
            *n += 1;
        }
    }
    Ok(new_page_id)
}

/// 源页有效区域：优先 CropBox（发票内容常只在裁剪区内），否则 MediaBox。返回 (x0,y0,x1,y1)。
fn src_page_box(src: &lopdf::Document, page_0based: u32) -> Result<(f32, f32, f32, f32), Box<dyn Error>> {
    let id = *src
        .get_pages()
        .get(&(page_0based + 1))
        .ok_or("源 PDF 页面不存在")?;
    let dict = src.get_object(id)?.as_dict()?;
    let key: &[u8] = if dict.get(b"CropBox").is_ok() { b"CropBox" } else { b"MediaBox" };
    let mb = dict.get(key)?.as_array()?;
    let (x0, y0, x1, y1) = (
        mb[0].as_float()?,
        mb[1].as_float()?,
        mb[2].as_float()?,
        mb[3].as_float()?,
    );
    Ok((x0, y0, x1, y1))
}

/// 返回页面内容流对象 id 列表，跟随引用链并展开数组（含 /Contents 间接引用→数组 的写法）。
/// 直接内联的 Stream 会作为新对象加入文档。
/// 数组必须保持原顺序：栈展开时逆序压入，弹出顺序即原顺序。
fn page_contents_ids(doc: &mut lopdf::Document, page_id: ObjectId) -> Result<Vec<ObjectId>, Box<dyn Error>> {
    let contents = {
        let page = doc.get_object(page_id)?.as_dict()?;
        page.get(b"Contents")?.clone()
    };
    let mut ids = Vec::new();
    let mut stack = vec![contents];
    while let Some(o) = stack.pop() {
        match o {
            Object::Reference(id) => stack.push(doc.get_object(id)?.clone()),
            Object::Stream(_) => ids.push(doc.add_object(o)),
            Object::Array(arr) => {
                // 逆序压栈，弹出时保持数组原顺序（栈是 LIFO）
                for x in arr.iter().rev() {
                    stack.push(x.clone());
                }
            }
            other => return Err(format!("非法 Contents: {other:?}").into()),
        }
    }
    if ids.is_empty() {
        return Err("页面无内容流".into());
    }
    Ok(ids)
}

/// 把拷贝的源页面按矩阵缩放/平移嵌入横版 A4，并把 /Contents 包进 `q cm ... Q`。
/// 内容流开头可能有多余的 Q（如票根以 Q 开头），会弹掉外层 q → 变换丢失 → 内容跑出页面，
/// 这里先按 q/Q 平衡，把多余的 Q 用前置 q 抵消。
/// `src_box` 为源页有效区域 (x0,y0,x1,y1)，用于计算缩放/平移（避免按整页 MediaBox 缩放导致内容过小）。
fn place_page(
    doc: &mut lopdf::Document,
    page_id: ObjectId,
    src_box: (f32, f32, f32, f32),
    rotate: bool,
    over_std: bool,
) -> Result<(), Box<dyn Error>> {
    let (bx0, by0, bx1, by1) = src_box;
    let bw = bx1 - bx0;
    let bh = by1 - by0;
    let margin = 8.0 * MM;
    let bottom_text_h = if over_std { 35.0 * MM } else { 20.0 * MM };
    let avail_w = PAGE_W - margin * 2.0;
    let avail_h = PAGE_H - margin * 2.0 - bottom_text_h;

    let matrix = if rotate {
        // 行程单竖版 → 横版，旋转 270°（等价原实现的 img.rotate270，即 90° 逆时针）
        // x' = -s*y + e, y' = s*x + f；有效区域左上 (bx0,by1) 转后应居中
        let s = (avail_w / bh).min(avail_h / bw);
        let e = PAGE_W / 2.0 + s * (by1 + by0) / 2.0;
        let f = PAGE_H / 2.0 - s * (bx1 + bx0) / 2.0;
        format!("0 {} {} 0 {} {}", s, -s, e, f)
    } else {
        let s = (avail_w / bw).min(avail_h / bh);
        let target_x = (PAGE_W - s * bw) / 2.0;
        let target_y = (PAGE_H - s * bh) / 2.0 + 5.0 * MM;
        let e = target_x - s * bx0;
        let f = target_y - s * by0;
        format!("{} 0 0 {} {} {}", s, s, e, f)
    };

    let page = doc.get_object_mut(page_id)?.as_dict_mut()?;
    page.set(
        b"MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(PAGE_W as i64),
            Object::Integer(PAGE_H as i64),
        ]),
    );
    // 源页 CropBox 等裁剪框按其原始坐标系定义，缩放后会造成错裁（文字丢失），移除让其使用 MediaBox
    page.remove(b"CropBox");
    page.remove(b"BleedBox");
    page.remove(b"TrimBox");
    page.remove(b"ArtBox");

    // 读取页面内容（单引用或数组），解压拼接后先平衡 q/Q，再包一层矩阵
    let contents_ids = page_contents_ids(doc, page_id)?;
    let mut body = Vec::new();
    for id in contents_ids {
        let mut stream = doc.get_object(id)?.as_stream()?.clone();
        let _ = stream.decompress();
        body.extend(stream.content);
    }
    // 用显式 clip 复刻源页的可见区域（CropBox/MediaBox）。源页移除 CropBox 后，
    // 若内容流含裁剪区外的元素（如滴滴发票底部隐藏的 "didi" 水印），会泄漏进导出页；
    // 这里在应用矩阵后按 src_box 裁剪（clip 与内容同坐标系，经矩阵映射到目标页），
    // 只保留发票可见内容。
    let mut wrapped = format!("q\n{matrix} cm\n{} {} {} {} re W n\n", bx0, by0, bw, bh).into_bytes();
    wrapped.extend(rebalance_q(&body));
    wrapped.extend(b"\nQ\n");
    let new_contents = doc.add_object(Stream::new(Default::default(), wrapped));
    doc.get_object_mut(page_id)?
        .as_dict_mut()?
        .set(b"Contents", Object::Reference(new_contents));
    Ok(())
}

/// 跳过字符串字面量与十六进制串，返回内容中 q/Q 的净深度。
fn q_net_depth(body: &[u8]) -> i32 {
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
                    // 是更长 token 的一部分（如 "quads"），跳过
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
    min_depth
}

/// 内容开头若有多余 Q（最小深度 < 0），前面补对应数量的 q。
fn rebalance_q(body: &[u8]) -> Vec<u8> {
    let min_depth = q_net_depth(body);
    if min_depth >= 0 {
        return body.to_vec();
    }
    let mut out = Vec::new();
    for _ in 0..(-min_depth) {
        out.extend(b"q\n");
    }
    out.extend_from_slice(body);
    out
}

/// 沿引用链找到实际对象 id（防 Reference→Reference）。
fn resolve_ref(doc: &lopdf::Document, mut id: ObjectId) -> Result<ObjectId, Box<dyn Error>> {
    for _ in 0..16 {
        match doc.get_object(id)? {
            Object::Reference(nid) => id = *nid,
            _ => return Ok(id),
        }
    }
    Err("引用链过深".into())
}

/// 把 CJK 字体注册进页面 /Resources/Font。
/// 处理 /Resources 与 /Font 可能是直接字典或间接引用的情况。
fn register_font(
    doc: &mut lopdf::Document,
    page_id: ObjectId,
    font: &cjk_font::CjkFont,
) -> Result<(), Box<dyn Error>> {
    let res_id: ObjectId = {
        let page = doc.get_object(page_id)?.as_dict()?;
        match page.get(b"Resources")? {
            Object::Reference(id) => *id,
            Object::Dictionary(d) => {
                let new_id = doc.add_object(Object::Dictionary(d.clone()));
                doc.get_object_mut(page_id)?
                    .as_dict_mut()?
                    .set(b"Resources", Object::Reference(new_id));
                new_id
            }
            other => return Err(format!("非法 Resources: {other:?}").into()),
        }
    };
    let res_id = resolve_ref(doc, res_id)?;
    if doc.get_object(res_id)?.as_dict().is_err() {
        return Err(format!("Resources 不是字典 (obj {res_id:?})").into());
    }

    // 规范化 /Font 为字典对象 id
    let font_dict_id: ObjectId = {
        let font_obj = doc.get_object_mut(res_id)?.as_dict_mut()?.get(b"Font").ok().cloned();
        match font_obj {
            None => {
                let new = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
                doc.get_object_mut(res_id)?
                    .as_dict_mut()?
                    .set(b"Font".to_vec(), Object::Reference(new));
                new
            }
            Some(Object::Dictionary(d)) => {
                let new = doc.add_object(Object::Dictionary(d));
                doc.get_object_mut(res_id)?
                    .as_dict_mut()?
                    .set(b"Font".to_vec(), Object::Reference(new));
                new
            }
            Some(Object::Reference(id)) => resolve_ref(doc, id)?,
            Some(other) => return Err(format!("非法 Font: {other:?}").into()),
        }
    };
    doc.get_object_mut(font_dict_id)?
        .as_dict_mut()?
        .set(
            cjk_font::FONT_KEY.as_bytes().to_vec(),
            Object::Reference(font.font_id),
        );
    Ok(())
}

fn append_content(
    doc: &mut lopdf::Document,
    page_id: ObjectId,
    ops: &str,
) -> Result<(), Box<dyn Error>> {
    let id = doc.add_object(Stream::new(Default::default(), ops.as_bytes().to_vec()));
    insert_content_stream(doc, page_id, id, true)?;
    Ok(())
}

/// 画一段文字；`center` 为 true 时按字体实际宽度水平居中到 x。
fn draw_text(
    doc: &mut lopdf::Document,
    page_id: ObjectId,
    font: &cjk_font::CjkFont,
    text: &str,
    size: f32,
    x: f32,
    y: f32,
    center: bool,
) -> Result<(), Box<dyn Error>> {
    if text.is_empty() {
        return Ok(());
    }
    let x0 = if center {
        x - cjk_font::text_width(font, text, size) / 2.0
    } else {
        x
    };
    append_content(
        doc,
        page_id,
        &cjk_font::text_content(font, text, size, x0, y),
    )
}

fn stroke_rect(
    doc: &mut lopdf::Document,
    page_id: ObjectId,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
) -> Result<(), Box<dyn Error>> {
    append_content(
        doc,
        page_id,
        &format!("q\n0.5 w\n{} {} {} {} re S\nQ\n", x0, y0, x1 - x0, y1 - y0),
    )
}

fn stroke_line(
    doc: &mut lopdf::Document,
    page_id: ObjectId,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
) -> Result<(), Box<dyn Error>> {
    append_content(
        doc,
        page_id,
        &format!("q\n0.5 w\n{} {} m {} {} l S\nQ\n", x0, y0, x1, y1),
    )
}

fn draw_blank(
    doc: &mut lopdf::Document,
    page_id: ObjectId,
    font: &cjk_font::CjkFont,
    payment: &str,
) -> Result<(), Box<dyn Error>> {
    let margin = 8.0 * MM;
    let bottom = 20.0 * MM;
    let x0 = margin;
    let y0 = bottom + margin;
    let x1 = PAGE_W - margin;
    let y1 = PAGE_H - margin;
    // 浅灰虚线框：标识"此处粘贴纸质票据"区域
    append_content(
        doc,
        page_id,
        &format!(
            "q\n0.5 w\n0.7 0.7 0.7 RG\n[4 3] 0 d\n{} {} {} {} re S\nQ\n",
            x0,
            y0,
            x1 - x0,
            y1 - y0
        ),
    )?;
    draw_text(
        doc,
        page_id,
        font,
        "（此处粘贴纸质票据）",
        16.0,
        PAGE_W / 2.0,
        (y0 + y1) / 2.0,
        true,
    )?;
    if !payment.is_empty() {
        draw_text(doc, page_id, font, payment, 14.0, PAGE_W / 2.0, 6.0 * MM, true)?;
    }
    Ok(())
}

fn draw_table(
    doc: &mut lopdf::Document,
    page_id: ObjectId,
    font: &cjk_font::CjkFont,
    rows: &[(usize, f64, String)],
) -> Result<(), Box<dyn Error>> {
    let col_w = [45.0 * MM, 65.0 * MM, 110.0 * MM];
    let total_w = col_w[0] + col_w[1] + col_w[2];
    let table_left = (PAGE_W - total_w) / 2.0;
    let row_h = 10.0 * MM;
    let header_top = PAGE_H - 25.0 * MM;
    let header_bot = header_top - row_h;
    let v1_x = table_left + col_w[0];
    let v2_x = table_left + col_w[0] + col_w[1];
    let h_margin = 5.0 * MM;
    let text_y = |bottom: f32| bottom + 2.5 * MM;

    stroke_rect(doc, page_id, table_left, header_bot, table_left + total_w, header_top)?;
    for vx in [v1_x, v2_x] {
        stroke_line(doc, page_id, vx, header_bot, vx, header_top)?;
    }
    draw_text(doc, page_id, font, "行程序号", 12.0, table_left + h_margin, text_y(header_bot), false)?;
    draw_text(doc, page_id, font, "行程金额", 12.0, v1_x + h_margin, text_y(header_bot), false)?;
    draw_text(doc, page_id, font, "支付单号", 12.0, v2_x + h_margin, text_y(header_bot), false)?;

    for (i, (seq, amt, pay_id)) in rows.iter().enumerate() {
        let row_top = header_bot - row_h * i as f32;
        let row_bot = row_top - row_h;
        stroke_rect(doc, page_id, table_left, row_bot, table_left + total_w, row_top)?;
        for vx in [v1_x, v2_x] {
            stroke_line(doc, page_id, vx, row_bot, vx, row_top)?;
        }
        draw_text(doc, page_id, font, &seq.to_string(), 11.0, table_left + h_margin, text_y(row_bot), false)?;
        draw_text(doc, page_id, font, &format!("{:.2}", amt), 11.0, v1_x + h_margin, text_y(row_bot), false)?;
        draw_text(doc, page_id, font, pay_id, 11.0, v2_x + h_margin, text_y(row_bot), false)?;
    }
    Ok(())
}

pub fn generate_comparison_image_pdf(
    match_results: &[MatchResult],
    invoice_dir: &str,
    output_path: &str,
    destination: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let mut specs: Vec<PageSpec> = Vec::new();
    let mut chars: BTreeSet<char> = BTreeSet::new();
    chars.extend(
        "微信支付宝单号：，。发票金额实报（此处粘贴纸质票据）行程序号支付、".chars(),
    );
    chars.extend('0'..='9');

    for result in match_results {
        match &result.invoice.source {
            InvoiceSource::Manual => {
                let payment = build_payment_text(result);
                chars.extend(payment.chars());
                specs.push(PageSpec::Blank { payment });
            }
            InvoiceSource::Pdf(pdf_path) => {
                let is_virtual = result.invoice.invoice_number.is_empty()
                    && matches!(result.invoice.category, InvoiceCategory::CityTransport)
                    && !result.invoice.itineraries.is_empty();
                if !is_virtual {
                    let payment = build_payment_text(result);
                    let over_std = build_over_std(result, destination);
                    chars.extend(payment.chars());
                    if let Some(ref o) = over_std {
                        chars.extend(o.chars());
                    }
                    specs.push(PageSpec::Invoice {
                        pdf: pdf_path.clone(),
                        page: 0,
                        rotate: false,
                        payment,
                        over_std,
                    });
                }
            }
            _ => {}
        }

        if matches!(result.invoice.category, InvoiceCategory::CityTransport)
            && !result.invoice.itineraries.is_empty()
        {
            if let Some(filename) = &result.invoice.itinerary_file {
                let pdf_path = Path::new(invoice_dir).join(filename);
                if pdf_path.exists() {
                    let src = lopdf::Document::load(&pdf_path)?;
                    let page_count = src.get_pages().len() as u32;
                    for p in 0..page_count {
                        specs.push(PageSpec::Invoice {
                            pdf: pdf_path.to_string_lossy().to_string(),
                            page: p,
                            rotate: true,
                            payment: String::new(),
                            over_std: None,
                        });
                    }
                }
            }

            let mut rows: Vec<(usize, f64, String)> = Vec::new();
            for (i, itin) in result.invoice.itineraries.iter().enumerate() {
                let pay_id = result
                    .payment_for_itinerary(i)
                    .map(|p| p.transaction_id.clone())
                    .unwrap_or_default();
                let amt_str = format!("{:.2}", itin.amount);
                let seq_str = (i + 1).to_string();
                chars.extend(pay_id.chars());
                chars.extend(amt_str.chars());
                chars.extend(seq_str.chars());
                rows.push((i + 1, itin.amount, pay_id));
            }
            if !rows.is_empty() {
                for chunk in rows.chunks(17) {
                    specs.push(PageSpec::Table {
                        rows: chunk.to_vec(),
                    });
                }
            }
        }
    }

    let mut doc = new_document();
    let font = cjk_font::embed(&mut doc, &chars)?;

    for spec in &specs {
        match spec {
            PageSpec::Invoice {
                pdf,
                page,
                rotate,
                payment,
                over_std,
            } => {
                let src = lopdf::Document::load(pdf)?;
                let src_box = src_page_box(&src, *page)?;
                let page_id = copy_page_safe(&mut doc, &src, *page)?;
                place_page(&mut doc, page_id, src_box, *rotate, over_std.is_some())?;
                register_font(&mut doc, page_id, &font)?;
                if let Some(ref ov) = over_std {
                    draw_text(
                        &mut doc,
                        page_id,
                        &font,
                        ov,
                        14.0,
                        PAGE_W / 2.0,
                        12.0 * MM,
                        true,
                    )?;
                }
                if !payment.is_empty() {
                    draw_text(
                        &mut doc,
                        page_id,
                        &font,
                        payment,
                        14.0,
                        PAGE_W / 2.0,
                        6.0 * MM,
                        true,
                    )?;
                }
            }
            PageSpec::Blank { payment } => {
                let page_id = create_blank_page(&mut doc, PAGE_W, PAGE_H)?;
                register_font(&mut doc, page_id, &font)?;
                draw_blank(&mut doc, page_id, &font, payment)?;
            }
            PageSpec::Table { rows } => {
                let page_id = create_blank_page(&mut doc, PAGE_W, PAGE_H)?;
                register_font(&mut doc, page_id, &font)?;
                draw_table(&mut doc, page_id, &font, rows)?;
            }
        }
    }

    doc.save(output_path)?;
    Ok(())
}
