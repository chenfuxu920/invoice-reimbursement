#![cfg(feature = "pdfplumber")]
//! 诊断测试：dump 043002200111_32092584.pdf 销售方行的原始 char 数据
//! 目的：定位"称"字坐标错误（应在 x≈47.50"名"之后，实际被报为 x≈133）

use pdfplumber::Pdf;
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

#[test]
fn dump_seller_line_raw_chars() {
    let pdf_path = format!("{DATA_DIR}\\市内交通\\043002200111_32092584.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    let bytes = std::fs::read(&pdf_path).unwrap();
    let pdf = match Pdf::open(&bytes, None) {
        Ok(p) => p,
        Err(e) => {
            let (p, _) = Pdf::open_with_repair(&bytes, None, None).unwrap();
            p
        }
    };

    let page_result = pdf.pages_iter().next().unwrap().unwrap();
    let chars = page_result.chars();

    // 销售方行 y≈299，过滤 y 在 290-310 范围
    let seller_chars: Vec<&pdfplumber::Char> = chars
        .iter()
        .filter(|c| c.bbox.top >= 290.0 && c.bbox.top <= 310.0)
        .collect();

    println!("\n=== 销售方行 (y 290-310) raw chars (content stream order) ===");
    println!("总数: {} chars\n", seller_chars.len());
    println!(
        "{:<4} {:<6} {:<8} {:<8} {:<8} {:<8} {:<6} {:<10} {:<6} {:<6} {:<6}",
        "idx", "text", "x0", "top", "x1", "bottom", "code", "font", "size", "tobj", "w"
    );
    for (i, ch) in seller_chars.iter().enumerate() {
        let w = ch.bbox.x1 - ch.bbox.x0;
        println!(
            "{:<4} {:<6} {:<8.2} {:<8.2} {:<8.2} {:<8.2} {:<6x} {:<10} {:<6.1} {:<6} {:<6.2}",
            i,
            ch.text,
            ch.bbox.x0,
            ch.bbox.top,
            ch.bbox.x1,
            ch.bbox.bottom,
            ch.char_code,
            ch.fontname,
            ch.size,
            ch.text_object_index,
            w,
        );
    }

    // 单独查找 [称] 字
    let cheng_chars: Vec<&pdfplumber::Char> = chars.iter().filter(|c| c.text == "称").collect();
    println!("\n=== 所有 [称] 字 (全文档) ===");
    for (i, ch) in cheng_chars.iter().enumerate() {
        println!(
            "  [{}] [称] x0={:.2} top={:.2} x1={:.2} bottom={:.2} font={} size={:.1} code={:x} tobj={}",
            i, ch.bbox.x0, ch.bbox.top, ch.bbox.x1, ch.bbox.bottom, ch.fontname, ch.size, ch.char_code, ch.text_object_index
        );
    }

    // 单独查找 [名] 字
    let ming_chars: Vec<&pdfplumber::Char> = chars.iter().filter(|c| c.text == "名").collect();
    println!("\n=== 所有 [名] 字 (全文档) ===");
    for (i, ch) in ming_chars.iter().enumerate() {
        println!(
            "  [{}] [名] x0={:.2} top={:.2} x1={:.2} bottom={:.2} font={} size={:.1} code={:x} tobj={}",
            i, ch.bbox.x0, ch.bbox.top, ch.bbox.x1, ch.bbox.bottom, ch.fontname, ch.size, ch.char_code, ch.text_object_index
        );
    }

    // 按 text_object_index 分组，看不同文本对象
    println!("\n=== 按 text_object_index 分组 (y 290-310) ===");
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<u32, Vec<&pdfplumber::Char>> = BTreeMap::new();
    for ch in &seller_chars {
        groups.entry(ch.text_object_index).or_default().push(ch);
    }
    for (tobj, group) in &groups {
        let text: String = group.iter().map(|c| c.text.as_str()).collect();
        let x0 = group
            .iter()
            .map(|c| c.bbox.x0)
            .fold(f64::INFINITY, f64::min);
        let x1 = group
            .iter()
            .map(|c| c.bbox.x1)
            .fold(f64::NEG_INFINITY, f64::max);
        println!(
            "  tobj={}: x=[{:.2}, {:.2}] text=\"{}\" ({} chars, fonts: {})",
            tobj,
            x0,
            x1,
            text,
            group.len(),
            group
                .iter()
                .map(|c| c.fontname.as_str())
                .collect::<std::collections::HashSet<_>>()
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // dump 完整 ctm 矩阵用于前几个销售方 chars
    println!("\n=== 前 40 个销售方 chars 的 render_mode 和 color ===");
    for (i, ch) in seller_chars.iter().take(40).enumerate() {
        let ns_color = ch
            .non_stroking_color
            .clone()
            .map(|c| format!("{:?}", c))
            .unwrap_or_else(|| "None".to_string());
        let s_color = ch
            .stroking_color
            .clone()
            .map(|c| format!("{:?}", c))
            .unwrap_or_else(|| "None".to_string());
        println!(
            "  [{}] '{}' code={:x} x0={:.2} render_mode={} non_stroke={} stroke={} font={} tobj={}",
            i,
            ch.text,
            ch.char_code,
            ch.bbox.x0,
            ch.render_mode,
            ns_color,
            s_color,
            ch.fontname,
            ch.text_object_index
        );
    }
}

#[test]
fn verify_seller_line_word_grouping() {
    let pdf_path = format!("{DATA_DIR}\\市内交通\\043002200111_32092584.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    let bytes = std::fs::read(&pdf_path).unwrap();
    let pdf = match Pdf::open(&bytes, None) {
        Ok(p) => p,
        Err(_) => {
            let (p, _) = Pdf::open_with_repair(&bytes, None, None).unwrap();
            p
        }
    };

    let page_result = pdf.pages_iter().next().unwrap().unwrap();
    let words = page_result.extract_words(&pdfplumber::WordOptions::default());

    // 找销售方行 (y≈299) 的 words
    let seller_words: Vec<&pdfplumber::Word> = words
        .iter()
        .filter(|w| w.bbox.top >= 290.0 && w.bbox.top <= 310.0)
        .collect();

    println!("\n=== 销售方行 words (颜色分拆后) ===");
    for (i, w) in seller_words.iter().enumerate() {
        let ns_color = w
            .non_stroking_color
            .clone()
            .map(|c| format!("{:?}", c))
            .unwrap_or_else(|| "None".to_string());
        println!(
            "  [{}] \"{}\" x0={:.2} top={:.2} x1={:.2} color={}",
            i, w.text, w.bbox.x0, w.bbox.top, w.bbox.x1, ns_color
        );
    }

    // 断言：棕色标签"名称:"和黑色值"长沙市轨道交通运营有限公司"应分开
    // 旧 bug：word grouping 混合产生 "称道：交通..." 之类乱序
    let all_seller_text: String = seller_words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    println!("\n  合并文本: \"{}\"", all_seller_text);

    // 关键断言：不应出现"称道"（棕色"称"混入黑色"道"）
    assert!(
        !all_seller_text.contains("称道"),
        "FAIL: '称' 和 '道' 不应在同一个 word 中（颜色分拆应分离它们）: {}",
        all_seller_text
    );

    // 关键断言：应能找到完整的值"长沙市轨道交通运营有限公司"（一个 word）
    let has_full_value = seller_words.iter().any(|w| {
        w.text.contains("长沙") && w.text.contains("轨道交通") && w.text.contains("有限公司")
    });
    assert!(
        has_full_value,
        "FAIL: 应找到完整的值 '长沙市轨道交通运营有限公司'，实际 words: {:?}",
        seller_words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
    );

    // 关键断言：标签"名"和"称:"存在（可能在不同 word，因棕色空格分隔）
    let has_name = seller_words.iter().any(|w| w.text.contains("名"));
    let has_cheng = seller_words.iter().any(|w| w.text.contains("称"));
    assert!(
        has_name && has_cheng,
        "FAIL: 应找到标签 '名' 和 '称:'，实际 words: {:?}",
        seller_words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
    );

    println!("\n  PASS: 标签和值正确分离");
}

#[test]
fn verify_full_extraction_contains_seller_name() {
    let pdf_path = format!("{DATA_DIR}\\市内交通\\043002200111_32092584.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    use invoice_reimbursement_lib::pdf::text_extractor::extract_pdf_column_aware;
    let extraction = extract_pdf_column_aware(&pdf_path).expect("extract should succeed");

    // 合并所有页的文本
    let all_text: String = extraction
        .pages
        .iter()
        .flat_map(|p| p.texts.iter())
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    println!("\n=== 完整提取文本（前 500 字符）===");
    println!("{}", &all_text.chars().take(500).collect::<String>());

    // 关键断言：销售方名称应作为完整字符串出现在提取文本中
    assert!(
        all_text.contains("长沙市轨道交通运营有限公司"),
        "FAIL: 提取文本应包含完整销售方名称 '长沙市轨道交通运营有限公司'，实际文本: {}",
        &all_text.chars().take(300).collect::<String>()
    );

    // 关键断言：不应出现"称道"（棕色标签混入黑色值的旧 bug）
    assert!(
        !all_text.contains("称道"),
        "FAIL: 不应出现 '称道'（颜色分组应防止标签混入值）"
    );

    println!("\n  PASS: 完整提取包含销售方名称 '长沙市轨道交通运营有限公司'");
}
