//! 中文文字嵌入：用 allsorts 子集化系统 CJK 字体，构建 Type0/Identity-H CID 字体。
//! ponytail: 只服务对照单的少量固定文字；通用排版需要完整字体引擎。

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use allsorts::binary::read::ReadScope;
use allsorts::font_data::FontData;
use allsorts::subset::{subset, CmapTarget, SubsetProfile};
use allsorts::tag;
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use ttf_parser::Face;

/// 字体在页面 /Resources 中使用的键名（避开 F1/F2 等常见冲突）
pub const FONT_KEY: &str = "Cjk";

const SUBSET_TAG: &str = "ABCDFG";

pub struct CjkFont {
    /// 文档内 Type0 字体对象 id（各页共享引用）
    pub font_id: ObjectId,
    /// char → CID（子集字体中的 glyph id）
    pub cid: BTreeMap<char, u16>,
    /// char → 宽度（1/1000 em）
    pub width_1000: BTreeMap<char, u16>,
}

fn find_system_font() -> Option<Vec<u8>> {
    let candidates: Vec<&str> = if cfg!(target_os = "windows") {
        vec![
            "C:/Windows/Fonts/simsun.ttc",
            "C:/Windows/Fonts/simhei.ttf",
            "C:/Windows/Fonts/msyh.ttc",
        ]
    } else {
        vec![
            "/usr/share/fonts/truetype/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ]
    };
    for p in candidates {
        if Path::new(p).exists() {
            if let Ok(bytes) = std::fs::read(p) {
                return Some(bytes);
            }
        }
    }
    None
}

/// 子集化系统 CJK 字体并嵌入文档，返回 char→CID 映射。
pub fn embed(doc: &mut Document, chars: &BTreeSet<char>) -> Result<CjkFont, String> {
    let font_bytes = find_system_font().ok_or("未找到系统 CJK 字体")?;
    let face = Face::parse(&font_bytes, 0).map_err(|e| format!("解析字体失败: {e:?}"))?;

    let mut gids: Vec<u16> = chars
        .iter()
        .filter_map(|c| face.glyph_index(*c).map(|g| g.0))
        .filter(|&g| g != 0)
        .collect();
    gids.sort();
    gids.dedup();
    if gids.is_empty() {
        return Err("文字不含可映射字形".to_string());
    }

    let font_file = ReadScope::new(&font_bytes)
        .read::<FontData>()
        .map_err(|e| format!("字体解析失败: {e}"))?;
    let provider = font_file
        .table_provider(0)
        .map_err(|e| format!("字体表读取失败: {e}"))?;

    let mut glyph_ids = vec![0u16];
    glyph_ids.extend_from_slice(&gids);
    let subsetted = subset(
        &provider,
        &glyph_ids,
        &SubsetProfile::Custom(vec![
            tag::CMAP,
            tag::HEAD,
            tag::HHEA,
            tag::HMTX,
            tag::MAXP,
            tag::NAME,
            tag::OS_2,
            tag::POST,
            tag::CVT,
            tag::FPGM,
            tag::PREP,
        ]),
        CmapTarget::Unicode,
    )
    .map_err(|e| format!("字体子集化失败: {e:?}"))?;

    // 从子集字体 cmap 取 char → 新 gid（即 CID，Identity-H 下 CID=GID）
    let sub_face = Face::parse(&subsetted, 0).map_err(|e| format!("解析子集字体失败: {e:?}"))?;
    let mut cid = BTreeMap::new();
    for &c in chars {
        if let Some(g) = sub_face.glyph_index(c) {
            if g.0 != 0 {
                cid.insert(c, g.0);
            }
        }
    }
    if cid.is_empty() {
        return Err("子集字体无可用字形".to_string());
    }

    let upem = face.units_per_em() as f32;
    let mut width_1000 = BTreeMap::new();
    for &c in chars {
        let w = face
            .glyph_index(c)
            .and_then(|g| face.glyph_hor_advance(g))
            .unwrap_or(0) as f32
            * 1000.0
            / upem;
        width_1000.insert(c, w.round() as u16);
    }

    let font_id = build_type0_font(doc, &subsetted, &face, &cid, &width_1000)?;
    Ok(CjkFont {
        font_id,
        cid,
        width_1000,
    })
}

fn build_type0_font(
    doc: &mut Document,
    subsetted: &[u8],
    face: &Face,
    cid: &BTreeMap<char, u16>,
    width_1000: &BTreeMap<char, u16>,
) -> Result<ObjectId, String> {
    let base_font = format!("{SUBSET_TAG}SimSun");

    let file_stream = Stream::new(
        dictionary! { "Length1" => subsetted.len() as i64 },
        subsetted.to_vec(),
    );
    let file_id = doc.add_object(file_stream);

    let upem = face.units_per_em() as f32;
    let scale = 1000.0 / upem;
    let bbox = face.global_bounding_box();
    let descriptor_id = doc.add_object(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => base_font.clone(),
        "Flags" => 4,
        "FontBBox" => Object::Array(vec![
            Object::Integer(bbox.x_min as i64),
            Object::Integer(bbox.y_min as i64),
            Object::Integer(bbox.x_max as i64),
            Object::Integer(bbox.y_max as i64),
        ]),
        "ItalicAngle" => 0,
        "Ascent" => (face.ascender() as f32 * scale).round() as i64,
        "Descent" => (face.descender() as f32 * scale).round() as i64,
        "CapHeight" => (face.capital_height().unwrap_or(face.ascender()) as f32 * scale).round() as i64,
        "StemV" => 0,
        "FontFile2" => file_id,
    });

    // /W 必须是 [c_first c_last w] 三元组（PDF 规范），写成成对的 [cid w] 会被误解析导致字符堆叠
    let mut w: Vec<Object> = Vec::new();
    for (&c, &cid_v) in cid {
        w.push(Object::Integer(cid_v as i64));
        w.push(Object::Integer(cid_v as i64));
        w.push(Object::Integer(*width_1000.get(&c).unwrap_or(&1000) as i64));
    }
    let cid_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => base_font.clone(),
        "CIDSystemInfo" => dictionary! {
            "Registry" => "Adobe",
            "Ordering" => "Identity",
            "Supplement" => 0
        },
        "FontDescriptor" => descriptor_id,
        "CIDToGIDMap" => "Identity",
        "DW" => 1000,
        "W" => Object::Array(w),
    });

    let to_unicode_id = doc.add_object(build_to_unicode(cid)?);

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => base_font,
        "Encoding" => "Identity-H",
        "DescendantFonts" => Object::Array(vec![Object::Reference(cid_id)]),
        "ToUnicode" => to_unicode_id,
    });
    Ok(font_id)
}

fn build_to_unicode(cid: &BTreeMap<char, u16>) -> Result<Stream, String> {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n\
         12 dict begin\n\
         begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n\
         /CMapType 2 def\n\
         1 begincodespacerange\n\
         <0000> <FFFF>\n\
         endcodespacerange\n",
    );
    s.push_str(&format!("{} beginbfchar\n", cid.len()));
    for (&c, &gid) in cid {
        let u = c as u32;
        let hex = if u <= 0xFFFF {
            format!("{u:04X}")
        } else {
            let v = u - 0x10000;
            format!("{:04X}{:04X}", 0xD800 + (v >> 10), 0xDC00 + (v & 0x3FF))
        };
        s.push_str(&format!("<{gid:04X}> <{hex}>\n"));
    }
    s.push_str(
        "endbfchar\n\
         endcmap\n\
         CMapName currentdict /CMap defineresource pop\n\
         end\n\
         end\n",
    );
    Ok(Stream::new(Default::default(), s.into_bytes()))
}

/// 生成绘制一段文字的内容流（`BT ... Tj ET`），缺字时跳过该字符。
pub fn text_content(font: &CjkFont, text: &str, size: f32, x: f32, y: f32) -> String {
    let mut hex = String::new();
    for c in text.chars() {
        if let Some(&cid_v) = font.cid.get(&c) {
            hex.push_str(&format!("{cid_v:04X}"));
        }
    }
    format!(
        "q\nBT\n/{} {} Tf\n{} {} Td\n<{}> Tj\nET\nQ\n",
        FONT_KEY, size, x, y, hex
    )
}

/// 文本宽度（用于居中/对齐），基于字体 hmtx。
pub fn text_width(font: &CjkFont, text: &str, size: f32) -> f32 {
    text.chars()
        .map(|c| *font.width_1000.get(&c).unwrap_or(&1000) as f32 * size / 1000.0)
        .sum()
}
