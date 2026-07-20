#![cfg(feature = "pdfplumber")]

use invoice_reimbursement_lib::pdf::debug_extract::debug_extract_texts;
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{DATA_DIR}\\{normalized}")
}

#[test]
fn analyze_invoice_pdf() {
    let pdf_path = data_path("市内交通\\043002200111_32092584.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: PDF not found at {pdf_path}");
        return;
    }

    println!("\n========== 发票 PDF 分析：043002200111_32092584.pdf ==========\n");
    println!("文件路径：{pdf_path}\n");

    let result = debug_extract_texts(&pdf_path, 200, None).expect("extract should succeed");

    for (page_idx, page) in result.pages.iter().enumerate() {
        println!("【页{}】尺寸：{}x{} 像素\n", page_idx + 1, page.width, page.height);
        
        println!("pdfplumber 提取的文字 ({} 项):", page.pdfplumber.len());
        // 按 Y 坐标分组，模拟行结构
        let mut rows: Vec<Vec<&invoice_reimbursement_lib::pdf::debug_extract::DebugTextItem>> = Vec::new();
        let line_height_threshold = 15.0; // 同一行的 Y 坐标差异阈值
        
        for item in &page.pdfplumber {
            let mut found_row = false;
            for row in &mut rows {
                if let Some(first) = row.first() {
                    if (item.y - first.y).abs() < line_height_threshold {
                        row.push(item);
                        found_row = true;
                        break;
                    }
                }
            }
            if !found_row {
                rows.push(vec![item]);
            }
        }
        
        // 按 Y 坐标排序行
        rows.sort_by(|a, b| {
            let ay = a.first().map(|i| i.y).unwrap_or(0.0);
            let by = b.first().map(|i| i.y).unwrap_or(0.0);
            ay.partial_cmp(&by).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        for (row_idx, row) in rows.iter().enumerate() {
            // 按 X 坐标排序行内元素
            let mut sorted_row: Vec<_> = row.clone();
            sorted_row.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
            
            let line_text: String = sorted_row
                .iter()
                .map(|i| format!("{}: '{}'", i.text.trim(), i.x))
                .collect::<Vec<_>>()
                .join(" | ");
            
            println!("  行{}: {}", row_idx + 1, line_text);
        }
        
        println!("\n单元格提取 ({} 项):", page.cells.len());
        for (cell_idx, cell) in page.cells.iter().take(50).enumerate() {
            println!("  单元格{}: [{:.0},{:.0} {:.0}x{:.0}] '{}'", 
                cell_idx + 1, cell.x, cell.y, cell.w, cell.h, cell.text.trim());
        }
        if page.cells.len() > 50 {
            println!("  ... 还有 {} 项", page.cells.len() - 50);
        }
    }
    
    println!("\n日志:");
    for log in &result.logs.pdfplumber {
        println!("  {log}");
    }
}

#[test]
fn analyze_didi_itinerary_pdf() {
    // Find the Didi itinerary PDF by iterating (encoding issues with Chinese paths)
    let didi_dir = format!("{DATA_DIR}\\行程单\\滴滴");
    let pdf_path = if let Ok(entries) = std::fs::read_dir(&didi_dir) {
        // Look for file containing "报销单 A" or just "A.pdf" with Chinese prefix
        entries
            .filter_map(|e| {
                e.ok().and_then(|entry| {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.contains("A.pdf") && name.len() > 10 {
                        Some(entry.path().to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
            })
            .next()
    } else {
        None
    };
    
    let pdf_path = match pdf_path {
        Some(p) => p,
        None => {
            eprintln!("SKIP: Cannot find Didi itinerary PDF in {didi_dir}");
            return;
        }
    };

    println!("\n========== 滴滴行程单 PDF 分析：滴滴出行行程报销单 A.pdf ==========\n");
    println!("文件路径：{pdf_path}\n");

    let result = debug_extract_texts(&pdf_path, 200, None).expect("extract should succeed");

    println!("总页数：{}\n", result.pages.len());

    for (page_idx, page) in result.pages.iter().enumerate() {
        println!("【页{}】尺寸：{}x{} 像素\n", page_idx + 1, page.width, page.height);
        
        println!("pdfplumber 提取的文字 ({} 项):", page.pdfplumber.len());
        // 按 Y 坐标分组，模拟行结构
        let mut rows: Vec<Vec<&invoice_reimbursement_lib::pdf::debug_extract::DebugTextItem>> = Vec::new();
        let line_height_threshold = 20.0; // 同一行的 Y 坐标差异阈值
        
        for item in &page.pdfplumber {
            let mut found_row = false;
            for row in &mut rows {
                if let Some(first) = row.first() {
                    if (item.y - first.y).abs() < line_height_threshold {
                        row.push(item);
                        found_row = true;
                        break;
                    }
                }
            }
            if !found_row {
                rows.push(vec![item]);
            }
        }
        
        // 按 Y 坐标排序行
        rows.sort_by(|a, b| {
            let ay = a.first().map(|i| i.y).unwrap_or(0.0);
            let by = b.first().map(|i| i.y).unwrap_or(0.0);
            ay.partial_cmp(&by).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        for (row_idx, row) in rows.iter().enumerate() {
            // 按 X 坐标排序行内元素
            let mut sorted_row: Vec<_> = row.clone();
            sorted_row.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
            
            let line_text: String = sorted_row
                .iter()
                .map(|i| format!("{}", i.text.trim()))
                .collect::<Vec<_>>()
                .join(" | ");
            
            println!("  行{}: {}", row_idx + 1, line_text);
        }
        
        println!("\n单元格提取 ({} 项):", page.cells.len());
        for (cell_idx, cell) in page.cells.iter().take(100).enumerate() {
            println!("  单元格{}: [{:.0},{:.0} {:.0}x{:.0}] '{}'", 
                cell_idx + 1, cell.x, cell.y, cell.w, cell.h, cell.text.trim());
        }
        if page.cells.len() > 100 {
            println!("  ... 还有 {} 项", page.cells.len() - 100);
        }
    }
    
    println!("\n日志:");
    for log in &result.logs.pdfplumber {
        println!("  {log}");
    }
}
