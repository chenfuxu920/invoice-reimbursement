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

/// PDF 规范的子集字体命名：6 字母大写标签 + '+' + 原字体名。
/// 缺 '+' 时部分引擎（如 Edge 内置的 Acrobat 阅读器）不按子集处理，
/// 会按名字替换系统字体 → 子集 GID 对上完整字体的字形 → 整页乱码。
fn subset_base_font() -> String {
    format!("{SUBSET_TAG}+SimSun")
}

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

    // 规范化子集字体：name 表 PostScript 名改为 BaseFont 一致 + 重算全部校验和。
    let subsetted = normalize_sfnt(&subsetted, &subset_base_font())?;

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
    let base_font = subset_base_font();

    let file_stream = Stream::new(
        dictionary! { "Length1" => subsetted.len() as i64 },
        subsetted.to_vec(),
    );
    let file_id = doc.add_object(file_stream);

    let upem = face.units_per_em() as f32;
    let scale = 1000.0 / upem;
    let bbox = face.global_bounding_box();
    // FontBBox 必须与 Ascent/Descent 一致使用 PDF 字形空间（1000/em）。
    // 直接抄字体原始 font units（SimSun upem=256）会让 BBox 缩成 1/4 字面，
    // 严格按 BBox 裁剪的引擎（Acrobat 系）会裁坏大部分汉字。
    let descriptor_id = doc.add_object(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => base_font.clone(),
        "Flags" => 4,
        "FontBBox" => Object::Array(vec![
            Object::Integer((bbox.x_min as f32 * scale).round() as i64),
            Object::Integer((bbox.y_min as f32 * scale).round() as i64),
            Object::Integer((bbox.x_max as f32 * scale).round() as i64),
            Object::Integer((bbox.y_max as f32 * scale).round() as i64),
        ]),
        "ItalicAngle" => 0,
        "Ascent" => (face.ascender() as f32 * scale).round() as i64,
        "Descent" => (face.descender() as f32 * scale).round() as i64,
        "CapHeight" => (face.capital_height().unwrap_or(face.ascender()) as f32 * scale).round() as i64,
        "StemV" => 0,
        "FontFile2" => file_id,
    });

    // /W 必须是 [c_first c_last w] 三元组（PDF 规范），写成成对的 [cid w] 会被误解析导致字符堆叠；
    // 条目按 CID 升序排列，避免个别解析器对乱序 W 的兼容问题。
    let mut widths: Vec<(u16, u16)> = cid
        .iter()
        .map(|(&c, &cid_v)| (cid_v, *width_1000.get(&c).unwrap_or(&1000)))
        .collect();
    widths.sort_unstable();
    let mut w: Vec<Object> = Vec::new();
    for (cid_v, width) in widths {
        w.push(Object::Integer(cid_v as i64));
        w.push(Object::Integer(cid_v as i64));
        w.push(Object::Integer(width as i64));
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
    // 规范限制每个 beginbfchar 块最多 100 条，超出需分块（BTreeMap 迭代已按 char 升序）
    let items: Vec<(&char, &u16)> = cid.iter().collect();
    for chunk in items.chunks(100) {
        s.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (&c, &gid) in chunk {
            let u = c as u32;
            let hex = if u <= 0xFFFF {
                format!("{u:04X}")
            } else {
                let v = u - 0x10000;
                format!("{:04X}{:04X}", 0xD800 + (v >> 10), 0xDC00 + (v & 0x3FF))
            };
            s.push_str(&format!("<{gid:04X}> <{hex}>\n"));
        }
        s.push_str("endbfchar\n");
    }
    s.push_str(
        "endcmap\n\
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

// ---------- sfnt 规范化 ----------

fn be16(b: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([b[off], b[off + 1]])
}
fn be32(b: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}
fn put32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_be_bytes());
}
fn put16(b: &mut [u8], off: usize, v: u16) {
    b[off..off + 2].copy_from_slice(&v.to_be_bytes());
}

/// 按规范算法对表的整 4 字节块求和（末尾不足补 0）。
fn table_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 4 <= data.len() {
        sum = sum.wrapping_add(be32(data, i));
        i += 4;
    }
    if i < data.len() {
        let mut last = [0u8; 4];
        last[..data.len() - i].copy_from_slice(&data[i..]);
        sum = sum.wrapping_add(u32::from_be_bytes(last));
    }
    sum
}

/// 重写 name 表中标识字体名的记录（family/full/PostScript 等）为 `new_name`。
/// 使嵌入字体内部名与 PDF BaseFont 一致，消除按名字替换的诱因。
fn rewrite_name_table(data: &[u8], new_name: &str) -> Result<Vec<u8>, String> {
    if data.len() < 6 {
        return Err("name 表过短".into());
    }
    let count = be16(data, 2) as usize;
    let storage = be16(data, 4) as usize;
    // (platform, encoding, lang, name_id, value)
    let mut records: Vec<(u16, u16, u16, u16, Vec<u8>)> = Vec::with_capacity(count);
    for r in 0..count {
        let ro = 6 + r * 12;
        if ro + 12 > data.len() {
            return Err("name 表记录越界".into());
        }
        let platform = be16(data, ro);
        let encoding = be16(data, ro + 2);
        let lang = be16(data, ro + 4);
        let name_id = be16(data, ro + 6);
        let len = be16(data, ro + 8) as usize;
        let off = be16(data, ro + 10) as usize;
        let so = storage + off;
        if so + len > data.len() {
            return Err("name 表字符串越界".into());
        }
        let original = data[so..so + len].to_vec();
        // 1=family 3=unique 4=full 6=PostScript 16=typographic family 17=subfamily
        let renamed = matches!(name_id, 1 | 3 | 4 | 6 | 16 | 17);
        let value = if renamed {
            match platform {
                0 | 3 => new_name.encode_utf16().flat_map(|u| u.to_be_bytes()).collect(),
                _ => new_name.as_bytes().to_vec(),
            }
        } else {
            original
        };
        records.push((platform, encoding, lang, name_id, value));
    }

    // 重组：记录头 + 字符串存储（每条按偶数对齐）
    let storage_new = 6 + count * 12;
    let mut out = vec![0u8; storage_new];
    put16(&mut out, 0, 0); // format 0
    put16(&mut out, 2, count as u16);
    put16(&mut out, 4, storage_new as u16);
    let mut blob: Vec<u8> = Vec::new();
    for (r, (platform, encoding, lang, name_id, value)) in records.iter().enumerate() {
        let off = blob.len();
        let ro = 6 + r * 12;
        put16(&mut out, ro, *platform);
        put16(&mut out, ro + 2, *encoding);
        put16(&mut out, ro + 4, *lang);
        put16(&mut out, ro + 6, *name_id);
        put16(&mut out, ro + 8, value.len() as u16);
        put16(&mut out, ro + 10, off as u16);
        blob.extend_from_slice(value);
        if blob.len() % 2 != 0 {
            blob.push(0);
        }
    }
    out.extend_from_slice(&blob);
    Ok(out)
}

/// 子集字体整体规范化：
/// 1. name 表字体名改为与 BaseFont 一致；
/// 2. 表目录按 tag 排序、searchRange 等字段规范填写；
/// 3. 每张表的目录校验和按规范重算；
/// 4. head.checkSumAdjustment 按规范重算（Acrobat 等引擎会校验）。
fn normalize_sfnt(data: &[u8], new_name: &str) -> Result<Vec<u8>, String> {
    if data.len() < 12 || be32(data, 0) != 0x00010000 {
        return Err("不是 TrueType (sfnt 1.0) 字体".into());
    }
    let num_tables = be16(data, 4) as usize;
    if 12 + num_tables * 16 > data.len() {
        return Err("表目录越界".into());
    }

    let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let e = 12 + i * 16;
        let mut tag = [0u8; 4];
        tag.copy_from_slice(&data[e..e + 4]);
        let off = be32(data, e + 8) as usize;
        let len = be32(data, e + 12) as usize;
        if off + len > data.len() {
            return Err(format!("表 {} 数据越界", String::from_utf8_lossy(&tag)));
        }
        tables.push((tag, data[off..off + len].to_vec()));
    }

    for (tag, content) in &mut tables {
        if tag == b"name" {
            *content = rewrite_name_table(content, new_name)?;
        }
        // head 的目录校验和按规范须以 checkSumAdjustment=0 计算
        if tag == b"head" && content.len() >= 12 {
            content[8..12].copy_from_slice(&[0, 0, 0, 0]);
        }
    }
    tables.sort_by_key(|(tag, _)| *tag);

    // 序列化：header + 目录 + 按 tag 顺序的表数据（4 字节对齐）
    let n = tables.len();
    let max_pow2 = usize::BITS - (n as u32).leading_zeros() - 1; // floor(log2(n))
    let search_range = 16 * (1usize << max_pow2) as u16;
    let entry_selector = max_pow2 as u16;
    let range_shift = (16 * n as u16).wrapping_sub(search_range);

    let dir_len = 12 + n * 16;
    let mut out = vec![0u8; dir_len];
    out[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    put16(&mut out, 4, n as u16);
    put16(&mut out, 6, search_range);
    put16(&mut out, 8, entry_selector);
    put16(&mut out, 10, range_shift);

    // 为各表分配偏移
    let mut entries: Vec<([u8; 4], u32, u32, u32)> = Vec::with_capacity(n); // tag, checksum, offset, len
    let mut payload: Vec<u8> = Vec::new();
    for (tag, content) in &tables {
        let offset = (dir_len + payload.len()) as u32;
        entries.push((*tag, table_checksum(content), offset, content.len() as u32));
        payload.extend_from_slice(content);
        while payload.len() % 4 != 0 {
            payload.push(0);
        }
    }
    for (i, (tag, checksum, offset, len)) in entries.iter().enumerate() {
        let e = 12 + i * 16;
        out[e..e + 4].copy_from_slice(tag);
        put32(&mut out, e + 4, *checksum);
        put32(&mut out, e + 8, *offset);
        put32(&mut out, e + 12, *len);
    }
    out.extend_from_slice(&payload);

    // head.checkSumAdjustment：整字体（该字段置 0）求和，0xB1B0AFBA - sum
    let head_idx = tables.iter().position(|(t, _)| t == b"head").ok_or("缺少 head 表")?;
    let head_off = entries[head_idx].2 as usize;
    if head_off + 54 > out.len() {
        return Err("head 表越界".into());
    }
    put32(&mut out, head_off + 8, 0);
    let whole = table_checksum(&out);
    put32(&mut out, head_off + 8, 0xB1B0AFBAu32.wrapping_sub(whole));

    Ok(out)
}
