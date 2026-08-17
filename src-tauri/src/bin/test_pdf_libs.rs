//! 诊断工具：对 data 目录下每个 PDF 逐一测试 pdfplumber 文本提取，
//! 报告成功/失败、文本长度、乱码率、首段文本。
//!
//! 用法: test_pdf_libs [pdf_dir]  (默认 data 目录)
//! 构建: cargo run --release --bin test_pdf_libs

use invoice_reimbursement_lib::pdf::text_extractor::is_garbled_text;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// 单库提取结果
struct LibResult {
    lib: &'static str,
    ok: bool,
    chars: usize,
    garbled: bool,
    sample: String,
    elapsed_ms: u128,
    error: Option<String>,
}

impl LibResult {
    fn fail(lib: &'static str, err: String, elapsed_ms: u128) -> Self {
        Self {
            lib,
            ok: false,
            chars: 0,
            garbled: false,
            sample: String::new(),
            elapsed_ms,
            error: Some(err),
        }
    }
    fn ok(lib: &'static str, text: String, elapsed_ms: u128) -> Self {
        let chars = text.chars().count();
        let garbled = is_garbled_text(&text, 0.3);
        let sample: String = text.chars().take(80).collect::<String>().replace('\n', " ");
        Self {
            lib,
            ok: true,
            chars,
            garbled,
            sample,
            elapsed_ms,
            error: None,
        }
    }
}

/// pdfplumber 独立测试（带坐标，flatten 为纯文本）
#[cfg(feature = "pdfplumber")]
fn test_pdfplumber(path: &str) -> LibResult {
    let t = Instant::now();
    use invoice_reimbursement_lib::pdf::text_extractor::extract_text_with_coords_flat;
    match extract_text_with_coords_flat(path) {
        Ok(items) => {
            let text: String = items
                .iter()
                .map(|i| i.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            LibResult::ok("pdfplumber", text, t.elapsed().as_millis())
        }
        Err(e) => LibResult::fail("pdfplumber", e, t.elapsed().as_millis()),
    }
}

#[cfg(not(feature = "pdfplumber"))]
fn test_pdfplumber(_path: &str) -> LibResult {
    LibResult::fail("pdfplumber", "feature disabled".to_string(), 0)
}

/// 递归收集目录下所有 PDF
fn collect_pdfs(dir: &Path) -> Vec<PathBuf> {
    let mut pdfs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pdfs.extend(collect_pdfs(&path));
            } else if path
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("pdf"))
            {
                pdfs.push(path);
            }
        }
    }
    pdfs.sort();
    pdfs
}

fn process_pdf(path: &Path) -> Vec<LibResult> {
    let path_str = path.to_string_lossy().to_string();
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("FILE: {}", name);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let mut results = Vec::new();

    // pdfplumber
    let r = test_pdfplumber(&path_str);
    print_result(&r);
    results.push(r);

    results
}

fn print_result(r: &LibResult) {
    let status = if r.ok { "OK" } else { "FAIL" };
    let garbled = if r.garbled { " [GARBLED]" } else { "" };
    if r.ok {
        println!(
            "  {:<14} {}  {:>5} chars  {:>4}ms{}  {}",
            r.lib, status, r.chars, r.elapsed_ms, garbled, r.sample
        );
    } else {
        println!(
            "  {:<14} {}  {:>5}        {:>4}ms  ERR: {}",
            r.lib,
            status,
            "",
            r.elapsed_ms,
            r.error.as_deref().unwrap_or("?")
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = if args.len() >= 2 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("data")
    };

    if !dir.is_dir() {
        // 尝试相对于 workspace root
        let alt = std::env::current_exe()
            .ok()
            .and_then(|p| p.ancestors().nth(4).map(|p| p.join("data")))
            .unwrap_or_else(|| PathBuf::from("data"));
        if alt.is_dir() {
            eprintln!("使用默认目录: {}", alt.display());
            let pdfs = collect_pdfs(&alt);
            run_all(&alt, &pdfs);
            return;
        }
        eprintln!("不是目录: {}", dir.display());
        std::process::exit(1);
    }

    let pdfs = collect_pdfs(&dir);
    run_all(&dir, &pdfs);
}

fn run_all(dir: &Path, pdfs: &[PathBuf]) {
    println!("共 {} 个 PDF，目录: {}", pdfs.len(), dir.display());

    // 统计：每个库的成功数、乱码数、总字符数
    let mut stats: std::collections::HashMap<&str, (usize, usize, usize)> =
        std::collections::HashMap::new();
    for lib in &["pdfplumber"] {
        stats.insert(lib, (0, 0, 0));
    }

    // 每个文件的各库结果摘要，用于最终汇总表
    let mut all_results: Vec<(String, Vec<LibResult>)> = Vec::new();

    for p in pdfs {
        let results = process_pdf(p);
        for r in &results {
            if let Some(s) = stats.get_mut(r.lib) {
                if r.ok {
                    s.0 += 1;
                    if r.garbled {
                        s.1 += 1;
                    }
                    s.2 += r.chars;
                }
            }
        }
        let name = p
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        all_results.push((name, results));
    }

    // ── 汇总表 ──
    println!("\n\n");
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                        汇总: 各库表现                            ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║ 库            │ 成功/总数  │ 乱码  │ 总字符  │ 说明              ║");
    println!("╠═══════════════╪═══════════╪═══════╪════════╪═══════════════════╣");

    let total = pdfs.len();
    for lib in &["pdfplumber"] {
        let s = stats.get(lib).unwrap_or(&(0, 0, 0));
        let desc = match *lib {
            "pdfplumber" => "坐标感知, Word级",
            _ => "",
        };
        println!(
            "║ {:<13} │ {:>3}/{:<5}  │ {:>5} │ {:>6} │ {} ║",
            lib, s.0, total, s.1, s.2, desc
        );
    }
    println!("╚══════════════════════════════════════════════════════════════════╝");

    // ── 哪些 PDF 只有特定库能提取 ──
    println!("\n── 独占成功（某库是唯一能提取的）──");
    let libs = ["pdfplumber"];
    let mut any_exclusive = false;
    for (name, results) in &all_results {
        let ok_libs: Vec<&str> = results.iter().filter(|r| r.ok).map(|r| r.lib).collect();
        if ok_libs.len() == 1 {
            any_exclusive = true;
            println!("  {} → 仅 {} 成功", name, ok_libs[0]);
        }
    }
    if !any_exclusive {
        println!("  (无独占场景 — 每个文件至少有 2 个库能提取，或全部失败)");
    }

    // ── 哪些 PDF 所有库都失败 ──
    println!("\n── 全部失败（纯扫描件，需 OCR）──");
    let mut any_all_fail = false;
    for (name, results) in &all_results {
        let ok_count = results.iter().filter(|r| r.ok).count();
        if ok_count == 0 {
            any_all_fail = true;
            println!("  {}", name);
        }
    }
    if !any_all_fail {
        println!("  (无 — 每个文件至少有 1 个库能提取)");
    }

    // ── 乱码情况 ──
    println!("\n── 乱码情况 ──");
    let mut any_garbled = false;
    for (name, results) in &all_results {
        let garbled_libs: Vec<&str> = results
            .iter()
            .filter(|r| r.ok && r.garbled)
            .map(|r| r.lib)
            .collect();
        if !garbled_libs.is_empty() {
            any_garbled = true;
            let clean_libs: Vec<&str> = results
                .iter()
                .filter(|r| r.ok && !r.garbled)
                .map(|r| r.lib)
                .collect();
            println!(
                "  {} → 乱码:{}  正常:{}",
                name,
                if garbled_libs.is_empty() {
                    "无".to_string()
                } else {
                    garbled_libs.join("+")
                },
                if clean_libs.is_empty() {
                    "无".to_string()
                } else {
                    clean_libs.join("+")
                }
            );
        }
    }
    if !any_garbled {
        println!("  (无乱码)");
    }

    // ── 矩阵表 ──
    println!("\n── 矩阵: 每文件 × 每库 (✓=OK ✗=fail G=garbled) ──");
    print!("  {:<50}", "FILE");
    for lib in &libs {
        print!(" {:>11}", lib);
    }
    println!();
    for (name, results) in &all_results {
        let short_name: String = name.chars().take(50).collect();
        print!("  {:<50}", short_name);
        for lib in &libs {
            let r = results.iter().find(|r| r.lib == *lib);
            let cell = match r {
                Some(r) if r.ok && r.garbled => "G".to_string(),
                Some(r) if r.ok => format!("{}", r.chars),
                Some(_) => "✗".to_string(),
                None => "?".to_string(),
            };
            print!(" {:>11}", cell);
        }
        println!();
    }
}
