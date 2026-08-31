#![cfg(feature = "pdfplumber")]
//! 火车票 CID 提取回归测试
//!
//! 验证 `data/火车票/25429165818005131893.pdf` 的 CID 文字提取不再产生韩文乱码。
//!
//! 背景：火车票 PDF 用 `GBK-EUC-H` 编码 Type0 CID 字体且无 `/ToUnicode`。
//! 修复前 `show_string_cid` 把 GBK 双字节当 2-byte CID，`char::from_u32(0xB9FA)`
//! = U+B9FA = "뻺"（韩文 Hangul），全文 91+11 个韩文乱码。
//! 修复（pdfplumber fork 24ffb68）用 encoding_rs 解码 GBK 字节为 Unicode 码点。

use invoice_reimbursement_lib::pdf::text_extractor::extract_pdf_column_aware;
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{DATA_DIR}\\{normalized}")
}

#[test]
fn train_ticket_cid_dump() {
    let pdf_path = data_path("火车票/25429165818005131893.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    println!("\n========== 火车票 CID 诊断 ==========");
    println!("文件: {pdf_path}");

    let extraction = match extract_pdf_column_aware(&pdf_path) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("提取失败: {e}");
            panic!("extract failed: {e}");
        }
    };

    println!("页数: {}", extraction.pages.len());

    // 文档级统计（跨页汇总）
    let mut total_ascii = 0usize;
    let mut total_cjk = 0usize;
    let mut total_hangul = 0usize; // U+AC00-U+D7AF (CID 当 Unicode 的典型症状)
    let mut total_hangul_compat = 0usize; // U+3130-U+318F
    let mut total_pua = 0usize;
    let mut total_replacement = 0usize;

    for (p_idx, page) in extraction.pages.iter().enumerate() {
        println!("\n--- Page {p_idx} ---");
        println!("文字条数: {}", page.texts.len());

        let all_text: String = page.texts.iter().map(|t| t.text.as_str()).collect();
        println!("全文 ({:?} chars): {}", all_text.chars().count(), all_text);

        for c in all_text.chars() {
            let code = c as u32;
            if code < 0x80 {
                total_ascii += 1;
            } else if (0x4E00..=0x9FFF).contains(&code) {
                total_cjk += 1;
            } else if (0xAC00..=0xD7AF).contains(&code) {
                total_hangul += 1;
            } else if (0x3130..=0x318F).contains(&code) {
                total_hangul_compat += 1;
            } else if (0xE000..=0xF8FF).contains(&code) {
                total_pua += 1;
            } else if code == 0xFFFD {
                total_replacement += 1;
            }
        }
    }

    println!("\n========== 文档级字符码点分布 ==========");
    println!("  ASCII:                    {total_ascii}");
    println!("  CJK 主区 (U+4E00-U+9FFF): {total_cjk}");
    println!("  韩文音节 (U+AC00-U+D7AF): {total_hangul}  ← CID 当 Unicode 的典型症状");
    println!("  韩文兼容 (U+3130-U+318F): {total_hangul_compat}");
    println!("  PUA (U+E000-U+F8FF):       {total_pua}");
    println!("  U+FFFD 替换:                {total_replacement}");

    // ponytail: 最小回归断言 — CID 当 Unicode bug 会让韩文音节+兼容字母暴涨；CJK 主区会塌缩
    assert_eq!(
        total_hangul, 0,
        "韩文音节必须为 0（CID 当 Unicode 症状）— got {total_hangul}"
    );
    assert_eq!(
        total_hangul_compat, 0,
        "韩文兼容字母必须为 0 — got {total_hangul_compat}"
    );
    assert!(
        total_cjk >= 50,
        "CJK 主区字符必须 ≥ 50（火车票应有大量中文）— got {total_cjk}"
    );

    println!("\n========== 回归断言通过 ==========\n");
}
