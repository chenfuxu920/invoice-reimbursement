#![cfg(feature = "pdfplumber")]
//! 分析发票 PDF 的字体、坐标、渲染顺序问题
//! 目标文件：data\市内交通\043002200111_32092584.pdf
//! 问题：标签"名 称:"和值"长沙市轨道交通运营有限公司"字体不同，zpdf 渲染重叠
//! Run: cargo test --features pdfplumber --test invoice_font_analysis_test -- --nocapture --ignored

use pdfplumber::{Pdf, WordOptions};
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{DATA_DIR}\\{normalized}")
}

#[test]
#[ignore]
fn analyze_changsha_subway_invoice() {
    let pdf_path = data_path("市内交通\\043002200111_32092584.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    eprintln!("\n========== 分析发票 PDF: {} ==========", pdf_path);
    
    let pdf = Pdf::open_file(&pdf_path, None).expect("open PDF");
    
    for page_result in pdf.pages_iter() {
        let page = match page_result {
            Ok(p) => p,
            Err(e) => {
                eprintln!("page error: {e}");
                continue;
            }
        };
        
        analyze_page(&page, page.page_number() + 1);
    }
}

fn analyze_page(page: &pdfplumber::Page, page_num: usize) {
    eprintln!("\n========== PAGE {} ==========", page_num);
    eprintln!("Page size: {}x{} points", page.width(), page.height());
    
    // 提取所有 words
    let words = page.extract_words(&WordOptions::default());
    
    eprintln!("\n=== 所有文字元素 ({} words) ===", words.len());
    
    // 寻找销售方区域（y ~299）的"名 称:"标签和"长沙市轨道交通运营有限公司"值
    eprintln!("\n=== 销售方名称区域分析 (y=299 附近) ===");
    let mut seller_name_words = Vec::new();
    for (i, word) in words.iter().enumerate() {
        let text = word.text.replace('\n', "\\n");
        // 销售方区域在 y=299 附近
        if (word.bbox.top - 299.25).abs() < 15.0 {
            seller_name_words.push((i, word));
            eprintln!("  [{i:3}] x={:7.2} y={:7.2} w={:6.2} h={:6.2} '{}'",
                word.bbox.x0, word.bbox.top, 
                word.bbox.x1 - word.bbox.x0, word.bbox.bottom - word.bbox.top,
                text
            );
        }
    }
    
    // 分析标签和值的边界
    eprintln!("\n=== 标签/值边界分析 ===");
    let label_words: Vec<_> = seller_name_words.iter()
        .filter(|(_, w)| w.text.contains("名") || w.text.contains("称") || w.text.contains(":") || w.text.contains(":"))
        .collect();
    let value_words: Vec<_> = seller_name_words.iter()
        .filter(|(_, w)| w.text.contains("长沙") || w.text.contains("轨道") || w.text.contains("交通") || w.text.contains("运营"))
        .collect();
    
    if let Some((label_idx, label_word)) = label_words.first() {
        eprintln!("  标签 '名 称:' [{}]: x={:.2}..{:.2} (w={:.2})", 
            label_idx, label_word.bbox.x0, label_word.bbox.x1, label_word.bbox.x1 - label_word.bbox.x0);
    }
    if let Some((value_idx, value_word)) = value_words.first() {
        eprintln!("  值 '长沙市轨道交通运营有限公司' [{}]: x={:.2}..{:.2} (w={:.2})", 
            value_idx, value_word.bbox.x0, value_word.bbox.x1, value_word.bbox.x1 - value_word.bbox.x0);
    }
    
    // 检查重叠
    eprintln!("\n=== 重叠检测 (销售方区域) ===");
    check_overlaps_in_region(&words, 290.0, 310.0);
    
    // 提取线条和矩形
    let lines = page.lines();
    let rects = page.rects();
    eprintln!("\n=== 图形元素 ===");
    eprintln!("  Lines: {}", lines.len());
    eprintln!("  Rects: {}", rects.len());
    
    // 打印销售方区域附近的矩形
    eprintln!("\n=== 销售方区域矩形 (y=288-357) ===");
    for (i, rect) in rects.iter().enumerate() {
        if rect.top >= 288.0 && rect.top <= 357.0 {
            let w = rect.x1 - rect.x0;
            let h = rect.bottom - rect.top;
            eprintln!("  [{i:3}] ({:.2}, {:.2}) w={:.2} h={:.2} lw={:.2} fill={}",
                rect.x0, rect.top, w, h, rect.line_width, rect.fill
            );
        }
    }
    
    // 提取 chars 详细信息（字体信息在 char 级别）
    eprintln!("\n=== 销售方区域 chars 详情 ===");
    for (i, word) in seller_name_words.iter() {
        eprintln!("  Word [{i}]: '{}' @ ({:.2}, {:.2})", word.text.trim(), word.bbox.x0, word.bbox.top);
        for (j, ch) in word.chars.iter().enumerate() {
            // 使用 byte 数组显示不可见字符
            let char_bytes: Vec<u8> = ch.text.bytes().collect();
            eprintln!("    char[{j}]: bytes={:?} font='{}' size={:.2} @ ({:.2}, {:.2}) w={:.2} h={:.2}",
                char_bytes, ch.fontname, ch.size,
                ch.bbox.x0, ch.bbox.top, ch.bbox.x1 - ch.bbox.x0, ch.bbox.bottom - ch.bbox.top
            );
        }
    }
    
    // 提取 PDF 中使用的唯一字体列表
    eprintln!("\n=== PDF 字体统计 ===");
    let mut font_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for word in words.iter() {
        for ch in word.chars.iter() {
            *font_counts.entry(ch.fontname.clone()).or_insert(0) += 1;
        }
    }
    for (font, count) in font_counts.iter() {
        eprintln!("  '{}': {} chars", font, count);
    }
}

fn check_overlaps(words: &[pdfplumber::Word]) {
    let mut overlaps = Vec::new();
    
    for (i, w1) in words.iter().enumerate() {
        for (j, w2) in words.iter().enumerate() {
            if i >= j { continue; }
            
            // 检查 Y 轴是否重叠（同一行）
            let y_overlap = !(w1.bbox.bottom < w2.bbox.top || w2.bbox.bottom < w1.bbox.top);
            
            if y_overlap {
                // 检查 X 轴是否重叠
                let x_overlap = !(w1.bbox.x1 < w2.bbox.x0 || w2.bbox.x1 < w1.bbox.x0);
                
                if x_overlap {
                    overlaps.push((i, j, w1, w2));
                }
            }
        }
    }
    
    if overlaps.is_empty() {
        eprintln!("  无重叠文字");
    } else {
        eprintln!("  发现 {} 处重叠:", overlaps.len());
        for (i, j, w1, w2) in overlaps.iter().take(20) {
            eprintln!("    [{i}] '{}' @ ({:.2}, {:.2}) 与 [{j}] '{}' @ ({:.2}, {:.2}) 重叠",
                w1.text.trim(), w1.bbox.x0, w1.bbox.top,
                w2.text.trim(), w2.bbox.x0, w2.bbox.top
            );
        }
    }
}

fn check_overlaps_in_region(words: &[pdfplumber::Word], y_min: f64, y_max: f64) {
    let region_words: Vec<_> = words.iter()
        .filter(|w| w.bbox.top >= y_min && w.bbox.top <= y_max)
        .collect();
    
    let mut overlaps = Vec::new();
    
    for (i, w1) in region_words.iter().enumerate() {
        for (j, w2) in region_words.iter().enumerate() {
            if i >= j { continue; }
            
            // 检查 Y 轴是否重叠（同一行）
            let y_overlap = !(w1.bbox.bottom < w2.bbox.top || w2.bbox.bottom < w1.bbox.top);
            
            if y_overlap {
                // 检查 X 轴是否重叠
                let x_overlap = !(w1.bbox.x1 < w2.bbox.x0 || w2.bbox.x1 < w1.bbox.x0);
                
                if x_overlap {
                    overlaps.push((i, j, w1, w2));
                }
            }
        }
    }
    
    if overlaps.is_empty() {
        eprintln!("  销售方区域无重叠文字");
    } else {
        eprintln!("  销售方区域发现 {} 处重叠:", overlaps.len());
        for (i, j, w1, w2) in overlaps.iter().take(20) {
            eprintln!("    [{i}] '{}' @ ({:.2}, {:.2}) 与 [{j}] '{}' @ ({:.2}, {:.2}) 重叠",
                w1.text.trim(), w1.bbox.x0, w1.bbox.top,
                w2.text.trim(), w2.bbox.x0, w2.bbox.top
            );
        }
    }
}
