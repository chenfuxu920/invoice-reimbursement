//! 解析 TrueType 字体文件，检查 Acrobat 引擎会校验的关键点：
//! 表目录校验和、head.checkSumAdjustment、name 表记录、必需表。
//! 用法: cargo run --bin inspect_font <ttf路径>

use std::fs;

fn be32(b: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}
fn be16(b: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([b[off], b[off + 1]])
}

/// 对表的整 4 字节块求和（PDF/OTF 规范算法）
fn table_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 4 <= data.len() {
        sum = sum.wrapping_add(be32(data, i));
        i += 4;
    }
    // 末尾不足 4 字节的部分补 0 计算
    if i < data.len() {
        let mut last = [0u8; 4];
        last[..data.len() - i].copy_from_slice(&data[i..]);
        sum = sum.wrapping_add(u32::from_be_bytes(last));
    }
    sum
}

fn main() {
    let path = std::env::args().nth(1).expect("用法: inspect_font <ttf>");
    let data = fs::read(&path).expect("读取失败");

    let num_tables = be16(&data, 4) as usize;
    println!("sfntVersion: 0x{:08X}, numTables: {num_tables}", be32(&data, 0));

    let mut total_sum = 0u32;
    for i in 0..num_tables {
        let off = 12 + i * 16;
        let tag = String::from_utf8_lossy(&data[off..off + 4]).to_string();
        let checksum = be32(&data, off + 4);
        let toff = be32(&data, off + 8) as usize;
        let tlen = be32(&data, off + 12) as usize;
        let actual = table_checksum(&data[toff..toff + tlen]);
        let ok = actual == checksum
            // head 特例：目录校验和按 checkSumAdjustment=0 计算，实际值 = 目录值 + adjustment
            || (tag == "head" && toff + 12 <= data.len()
                && actual.wrapping_add(be32(&data, toff + 8).wrapping_neg()) == checksum);
        if !ok || matches!(tag.as_str(), "head" | "name" | "maxp" | "hhea" | "cmap" | "post" | "OS/2") {
            println!(
                "  {tag:8} off={toff:6} len={tlen:6} checksum={checksum:08X} actual={actual:08X} {}",
                if ok { "OK" } else { "**MISMATCH**" }
            );
        }
        total_sum = total_sum.wrapping_add(actual);
        if tag == "head" && toff + 54 <= data.len() {
            let adjust = be32(&data, toff + 8);
            let entry_selector = be16(&data, toff + 12);
            let index_to_loc = be16(&data, toff + 18);
            // checkSumAdjustment 计算：整个字体文件按 head.checkSumAdjustment 置 0 后求和，取 0xB1B0AFBA - sum
            let mut file_sum = 0u32;
            let mut j = 0;
            let mut buf = data.clone();
            buf[toff + 8..toff + 12].fill(0);
            while j + 4 <= buf.len() {
                file_sum = file_sum.wrapping_add(be32(&buf, j));
                j += 4;
            }
            if j < buf.len() {
                let mut last = [0u8; 4];
                last[..buf.len() - j].copy_from_slice(&buf[j..]);
                file_sum = file_sum.wrapping_add(u32::from_be_bytes(last));
            }
            let expected = 0xB1B0AFBAu32.wrapping_sub(file_sum);
            println!(
                "    head: unitsPerEm={units} checkSumAdjustment={adjust:08X} expected={expected:08X} {} indexToLocFormat={index_to_loc} entrySelector={entry_selector}",
                if adjust == expected { "OK" } else { "**BAD**" },
                units = be16(&data, toff + 18 + 0) // placeholder, replaced below
            );
        }
        if tag == "name" {
            let count = be16(&data, toff + 2) as usize;
            let str_off_base = toff + be16(&data, toff + 4) as usize;
            println!("    name: count={count}");
            for r in 0..count.min(30) {
                let roff = toff + 6 + r * 12;
                let platform = be16(&data, roff);
                let encoding = be16(&data, roff + 2);
                let lang = be16(&data, roff + 4);
                let name_id = be16(&data, roff + 6);
                let len = be16(&data, roff + 8) as usize;
                let soff = str_off_base + be16(&data, roff + 10) as usize;
                let raw = &data[soff..soff + len];
                let text = if platform == 3 || platform == 0 {
                    String::from_utf16_lossy(
                        &raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>(),
                    )
                } else {
                    String::from_utf8_lossy(raw).to_string()
                };
                println!("      p{platform}e{encoding} l{lang} id{name_id}: {text:?}");
            }
        }
        if tag == "maxp" {
            println!("    maxp: numGlyphs={}", be16(&data, toff + 4));
        }
        if tag == "hhea" {
            println!("    hhea: numberOfHMetrics={}", be16(&data, toff + 34));
        }
        if tag == "cmap" {
            let n = be16(&data, toff + 2) as usize;
            println!("    cmap: {n} subtables");
            for s in 0..n {
                let so = toff + 4 + s * 8;
                println!(
                    "      platform={} encoding={} offset={}",
                    be16(&data, so),
                    be16(&data, so + 2),
                    be32(&data, so + 4)
                );
            }
        }
    }
    let _ = total_sum;
}
