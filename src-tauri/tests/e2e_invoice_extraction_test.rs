/// 端到端发票信息提取快照测试
///
/// 对 data 各目录的所有发票/行程单进行完整流水线提取，
/// 首次运行自动生成基准快照 `data/expected_extraction.json`，
/// 后续运行与快照比对，验证每张发票的字段值精确性。
///
/// 快照包含每个文件的：
///   - data  : 结构化提取结果（Invoice/ItineraryDoc 字段，用于自动比对）
///   - raw_text : 原始提取文字（OCR/pdfplumber 输出，供 LLM 人工核对提取是否正确）
///   - error : 解析失败时的错误信息（成功时为 null）
///
/// ## 运行方式
///
/// 首次生成快照：
///   cargo test --features pdfplumber --test e2e_invoice_extraction_test -- --ignored --nocapture
///
/// 强制重新生成快照（解析逻辑升级后）：
///   $env:UPDATE_SNAPSHOT=1
///   cargo test --features pdfplumber --test e2e_invoice_extraction_test -- --ignored --nocapture
///
/// 比对验证（默认）：
///   cargo test --features pdfplumber --test e2e_invoice_extraction_test -- --ignored --nocapture
use invoice_reimbursement_lib::{
    models::invoice::{Invoice, InvoiceCategory, InvoiceSource},
    ocr::OcrEngine,
    pdf::invoice_pipeline::{
        extract_text_with_coords_or_fallback, parse_invoice_from_pdf, parse_itinerary_from_pdf,
        ExtractionConfig, ItineraryDoc,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MODELS_DIR: &str = "models";
const SNAPSHOT_PATH: &str = "../data/expected_extraction.json";
const FLOAT_TOLERANCE: f64 = 0.01;

// ─── 快照数据结构 ───────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    #[serde(default)]
    generated_at: String,
    invoices: HashMap<String, Vec<SnapshotEntry>>,
    itineraries: HashMap<String, Vec<SnapshotEntry>>,
}

#[derive(Serialize, Deserialize)]
struct SnapshotEntry {
    file: String,
    /// 结构化提取结果（解析失败时为 None）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
    /// 原始提取文字（OCR/pdfplumber 输出，按行拼接，供 LLM 核对）
    #[serde(default)]
    raw_text: String,
    /// 解析失败时的错误信息
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// ─── 测试用例配置 ───────────────────────────────────────────

struct DirCase {
    dir_label: &'static str,
    path: &'static str,
    #[allow(dead_code)]
    expected_category: InvoiceCategory,
}

fn build_invoice_cases() -> Vec<DirCase> {
    vec![
        DirCase { dir_label: "住宿", path: "../data/住宿", expected_category: InvoiceCategory::Hotel },
        DirCase { dir_label: "市内交通", path: "../data/市内交通", expected_category: InvoiceCategory::CityTransport },
        DirCase { dir_label: "机票", path: "../data/机票", expected_category: InvoiceCategory::Flight },
        DirCase { dir_label: "火车票", path: "../data/火车票", expected_category: InvoiceCategory::Train },
        DirCase { dir_label: "保险", path: "../data/保险", expected_category: InvoiceCategory::Insurance },
        DirCase { dir_label: "通行费", path: "../data/通行费", expected_category: InvoiceCategory::Toll },
        DirCase { dir_label: "退改签", path: "../data/退改签", expected_category: InvoiceCategory::TicketChange },
        DirCase { dir_label: "未分类", path: "../data/未分类", expected_category: InvoiceCategory::Other },
    ]
}

fn build_itinerary_cases() -> Vec<DirCase> {
    vec![
        DirCase { dir_label: "滴滴", path: "../data/行程单/滴滴", expected_category: InvoiceCategory::CityTransport },
        DirCase { dir_label: "天府通", path: "../data/行程单/天府通", expected_category: InvoiceCategory::CityTransport },
        DirCase { dir_label: "高德", path: "../data/行程单/高德", expected_category: InvoiceCategory::CityTransport },
    ]
}

// ─── 主测试入口 ─────────────────────────────────────────────

#[test]
#[ignore]
fn e2e_extraction_snapshot_test() {
    let mut engine = OcrEngine::new(MODELS_DIR).expect("OCR init failed");
    let config = ExtractionConfig::default();

    // 1. 解析所有发票目录（逐文件：原始文字 + 结构化解析）
    let mut actual_invoices: HashMap<String, Vec<SnapshotEntry>> = HashMap::new();
    for case in build_invoice_cases() {
        println!("\n═══════════════════════════════════════════");
        println!("  发票目录: {}", case.dir_label);
        println!("═══════════════════════════════════════════");
        let entries = process_invoice_dir(case.path, &mut engine, &config);
        let ok = entries.iter().filter(|e| e.data.is_some()).count();
        let fail = entries.iter().filter(|e| e.data.is_none()).count();
        println!("  解析: {} 个文件, {} 成功, {} 失败", entries.len(), ok, fail);
        actual_invoices.insert(case.dir_label.to_string(), entries);
    }

    // 2. 解析所有行程单目录
    let mut actual_itineraries: HashMap<String, Vec<SnapshotEntry>> = HashMap::new();
    for case in build_itinerary_cases() {
        println!("\n═══════════════════════════════════════════");
        println!("  行程单目录: {}", case.dir_label);
        println!("═══════════════════════════════════════════");
        let entries = process_itinerary_dir(case.path, &mut engine);
        let ok = entries.iter().filter(|e| e.data.is_some()).count();
        let fail = entries.iter().filter(|e| e.data.is_none()).count();
        println!("  解析: {} 个文件, {} 成功, {} 失败", entries.len(), ok, fail);
        actual_itineraries.insert(case.dir_label.to_string(), entries);
    }

    // 3. 快照生成 or 比对
    let snapshot_exists = Path::new(SNAPSHOT_PATH).exists();
    let force_update = std::env::var("UPDATE_SNAPSHOT").ok().as_deref() == Some("1");

    if !snapshot_exists || force_update {
        let snapshot = Snapshot {
            version: 1,
            generated_at: chrono::Utc::now().to_rfc3339(),
            invoices: actual_invoices,
            itineraries: actual_itineraries,
        };
        let json = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");
        std::fs::write(SNAPSHOT_PATH, json).expect("write snapshot");

        let inv_count: usize = snapshot.invoices.values().map(|v| v.len()).sum();
        let itin_count: usize = snapshot.itineraries.values().map(|v| v.len()).sum();
        println!("\n═══════════════════════════════════════════");
        println!("  ✓ 已生成基准快照: {}", SNAPSHOT_PATH);
        println!("    发票文件: {} 个, 行程单文件: {} 个", inv_count, itin_count);
        println!("\n  每个文件含 data(结构化) + raw_text(原始文字) + error(失败原因)");
        println!("  请人工/LLM 核对 raw_text 与 data 是否吻合，确认后再次运行即比对。");
        println!("  解析逻辑升级后，用 UPDATE_SNAPSHOT=1 重新生成。");
        return;
    }

    // 4. 比对快照
    let snapshot_content = std::fs::read_to_string(SNAPSHOT_PATH).expect("read snapshot");
    let snapshot: Snapshot = serde_json::from_str(&snapshot_content).expect("parse snapshot");

    let mut failures: Vec<String> = vec![];

    // 比对发票
    for (dir, expected_entries) in &snapshot.invoices {
        let actual = actual_invoices.get(dir).map(|v| v.as_slice()).unwrap_or(&[]);
        compare_dir_entries(dir, expected_entries, actual, &mut failures);
    }
    for dir in actual_invoices.keys() {
        if !snapshot.invoices.contains_key(dir) {
            failures.push(format!("发票目录 [{}] 在快照中不存在（需 UPDATE_SNAPSHOT=1 重新生成）", dir));
        }
    }

    // 比对行程单
    for (dir, expected_entries) in &snapshot.itineraries {
        let actual = actual_itineraries.get(dir).map(|v| v.as_slice()).unwrap_or(&[]);
        compare_dir_entries(dir, expected_entries, actual, &mut failures);
    }
    for dir in actual_itineraries.keys() {
        if !snapshot.itineraries.contains_key(dir) {
            failures.push(format!("行程单目录 [{}] 在快照中不存在（需 UPDATE_SNAPSHOT=1 重新生成）", dir));
        }
    }

    // 汇总
    println!("\n═══════════════════════════════════════════");
    if failures.is_empty() {
        println!("  ✓ 快照比对通过");
    } else {
        eprintln!("  ❌ 快照比对失败 ({} 项):", failures.len());
        for f in &failures { eprintln!("    - {}", f); }
        eprintln!("\n  如解析逻辑有意升级，用 UPDATE_SNAPSHOT=1 重新生成快照。");
        panic!("{} 项快照比对失败", failures.len());
    }
}

// ─── 目录处理 ───────────────────────────────────────────────

/// 处理发票目录：逐文件提取原始文字 + 解析结构化数据
fn process_invoice_dir(
    dir: &str,
    engine: &mut OcrEngine,
    config: &ExtractionConfig,
) -> Vec<SnapshotEntry> {
    let mut entries = vec![];
    let mut pdf_files = list_pdf_files(dir);
    pdf_files.sort();

    for path in pdf_files {
        let fname = Path::new(&path).file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path)
            .to_string();

        // 提取原始文字
        let raw_text = extract_raw_text(&path, engine);

        // 解析结构化数据
        match parse_invoice_from_pdf(&path, engine, config) {
            Ok(inv) => {
                println!("  ✓ {}  金额={:.2}  类别={:?}  日期={}  原始文字={}字",
                    fname, inv.amount, inv.category, inv.date, raw_text.chars().count());
                entries.push(SnapshotEntry {
                    file: fname,
                    data: Some(normalize_invoice_value(&inv)),
                    raw_text,
                    error: None,
                });
            }
            Err(e) => {
                eprintln!("  ✗ {}  解析失败: {}  原始文字={}字", fname, e, raw_text.chars().count());
                entries.push(SnapshotEntry {
                    file: fname,
                    data: None,
                    raw_text,
                    error: Some(e),
                });
            }
        }
    }
    entries
}

/// 处理行程单目录：逐文件提取原始文字 + 解析结构化数据
fn process_itinerary_dir(dir: &str, engine: &mut OcrEngine) -> Vec<SnapshotEntry> {
    let mut entries = vec![];
    let mut pdf_files = list_pdf_files(dir);
    pdf_files.sort();

    for path in pdf_files {
        let fname = Path::new(&path).file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path)
            .to_string();

        let raw_text = extract_raw_text(&path, engine);

        match parse_itinerary_from_pdf(&path, engine) {
            Ok(doc) => {
                println!("  ✓ {}  行程数={}  合计={:.2}  原始文字={}字",
                    fname, doc.itineraries.len(), doc.total_amount, raw_text.chars().count());
                entries.push(SnapshotEntry {
                    file: fname,
                    data: Some(normalize_itinerary_doc_value(&doc)),
                    raw_text,
                    error: None,
                });
            }
            Err(e) => {
                eprintln!("  ✗ {}  解析失败: {}  原始文字={}字", fname, e, raw_text.chars().count());
                entries.push(SnapshotEntry {
                    file: fname,
                    data: None,
                    raw_text,
                    error: Some(e),
                });
            }
        }
    }
    entries
}

// ─── 辅助函数 ───────────────────────────────────────────────

/// 列出目录下所有 PDF 文件（返回完整路径）
fn list_pdf_files(dir: &str) -> Vec<String> {
    let mut files = vec![];
    if !Path::new(dir).exists() {
        eprintln!("  [warn] 目录不存在: {}", dir);
        return files;
    }
    for entry in std::fs::read_dir(dir).expect("read_dir 失败") {
        let entry = entry.expect("entry 失败");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("pdf") {
            if let Some(p) = path.to_str() {
                files.push(p.to_string());
            }
        }
    }
    files
}

/// 提取 PDF 的原始文字（OCR/pdfplumber 输出，按行拼接）
fn extract_raw_text(pdf_path: &str, engine: &mut OcrEngine) -> String {
    match extract_text_with_coords_or_fallback(pdf_path, engine) {
        Ok(items) => {
            items.iter().map(|i| i.text.as_str()).collect::<Vec<_>>().join("\n")
        }
        Err(e) => {
            eprintln!("  [warn] 原始文字提取失败 {}: {}", pdf_path, e);
            String::new()
        }
    }
}

/// 从 Invoice.source 提取文件名（basename）
#[allow(dead_code)]
fn extract_filename(inv: &Invoice) -> String {
    let path = match &inv.source {
        InvoiceSource::Pdf(p) | InvoiceSource::Photo(p) | InvoiceSource::Link(p) => p,
        InvoiceSource::Manual => return "(manual)".to_string(),
    };
    Path::new(path).file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string()
}

/// 序列化 Invoice 并剥离不稳定字段（id 自增、source 含绝对路径）
fn normalize_invoice_value(inv: &Invoice) -> Value {
    let mut v = serde_json::to_value(inv).expect("serialize invoice");
    if let Value::Object(ref mut map) = v {
        map.remove("id");
        map.remove("source");
        // itinerary_file 可能含路径，归一化为 basename
        if let Some(Value::String(ref mut p)) = map.get_mut("itinerary_file") {
            if !p.is_empty() {
                *p = Path::new(p).file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(p)
                    .to_string();
            }
        }
    }
    v
}

/// 序列化 ItineraryDoc 并剥离 file_name（与外层 entry 的 file 重复）
fn normalize_itinerary_doc_value(doc: &ItineraryDoc) -> Value {
    let mut v = serde_json::to_value(doc).expect("serialize itinerary doc");
    if let Value::Object(ref mut map) = v {
        map.remove("file_name");
    }
    v
}

// ─── 快照比对 ───────────────────────────────────────────────

/// 比对单个目录下的所有文件条目
fn compare_dir_entries(
    dir: &str,
    expected: &[SnapshotEntry],
    actual: &[SnapshotEntry],
    fails: &mut Vec<String>,
) {
    let actual_map: HashMap<&str, &SnapshotEntry> = actual.iter()
        .map(|e| (e.file.as_str(), e))
        .collect();

    let exp_files: HashSet<&str> = expected.iter()
        .map(|e| e.file.as_str())
        .collect();

    // 比对快照中已有的文件
    for exp in expected {
        match actual_map.get(exp.file.as_str()) {
            Some(act) => {
                // 比对结构化数据（data 字段）
                match (&exp.data, &act.data) {
                    (Some(exp_val), Some(act_val)) => {
                        compare_values(
                            exp_val, act_val,
                            &format!("[{}/{}]", dir, exp.file),
                            fails,
                        );
                    }
                    (Some(_), None) => {
                        fails.push(format!(
                            "[{}/{}] 解析回归：快照有结构化数据，实际解析失败: {}",
                            dir, exp.file, act.error.as_deref().unwrap_or("(unknown)")
                        ));
                    }
                    (None, Some(_)) => {
                        fails.push(format!(
                            "[{}/{}] 新增解析成功：快照记录为失败，实际已修复（需 UPDATE_SNAPSHOT=1）",
                            dir, exp.file
                        ));
                    }
                    (None, None) => {
                        // 两端都失败：可接受（除非错误信息类型不同，这里不深究）
                    }
                }
                // raw_text 不参与自动比对（供 LLM 人工核对）
            }
            None => {
                fails.push(format!(
                    "[{}/{}] 实际结果缺失该文件（文件被删除或重命名）",
                    dir, exp.file
                ));
            }
        }
    }
    // 实际多出的文件（快照中没有）
    for act in actual {
        if !exp_files.contains(act.file.as_str()) {
            fails.push(format!(
                "[{}/{}] 实际结果多出该文件（快照中无，需 UPDATE_SNAPSHOT=1）",
                dir, act.file
            ));
        }
    }
}

/// 递归比对两个 JSON Value，数值用容差，字符串精确匹配
fn compare_values(expected: &Value, actual: &Value, path: &str, fails: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Number(e), Value::Number(a)) => {
            let ef = e.as_f64().unwrap_or(0.0);
            let af = a.as_f64().unwrap_or(0.0);
            if (ef - af).abs() > FLOAT_TOLERANCE {
                fails.push(format!("{}: 数值不符 预期={} 实际={} (容差{})", path, ef, af, FLOAT_TOLERANCE));
            }
        }
        (Value::String(e), Value::String(a)) => {
            if e != a {
                fails.push(format!("{}: 字符串不符\n    预期='{}'\n    实际='{}'", path, e, a));
            }
        }
        (Value::Object(e), Value::Object(a)) => {
            for (k, ev) in e {
                match a.get(k) {
                    Some(av) => compare_values(ev, av, &format!("{}.{}", path, k), fails),
                    None => fails.push(format!("{}.{}: 实际结果缺失字段（预期有值）", path, k)),
                }
            }
            for k in a.keys() {
                if !e.contains_key(k) {
                    fails.push(format!("{}.{}: 实际结果多出字段（快照无，需 UPDATE_SNAPSHOT=1）", path, k));
                }
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            if e.len() != a.len() {
                fails.push(format!("{}: 数组长度不符 预期={} 实际={}", path, e.len(), a.len()));
            }
            for (i, (ev, av)) in e.iter().zip(a.iter()).enumerate() {
                compare_values(ev, av, &format!("{}[{}]", path, i), fails);
            }
        }
        (Value::Null, Value::Null) => {}
        (Value::Bool(e), Value::Bool(a)) => {
            if e != a {
                fails.push(format!("{}: 布尔不符 预期={} 实际={}", path, e, a));
            }
        }
        _ => {
            fails.push(format!("{}: 类型不符 预期={} 实际={}", path, expected, actual));
        }
    }
}

// ponytail: 数值容差 0.01 是 OCR 物理世界数据所需——真实 OCR 可能有浮点舍入误差。
// 升级路径：若需要更严格，可按字段类型分级容差（金额 0.01，坐标 1.0）。
// ponytail: raw_text 不参与自动比对——它供 LLM/人工核对提取是否正确，
// 若文本提取逻辑变化导致 raw_text 变化，会反映在 data 比对上。
