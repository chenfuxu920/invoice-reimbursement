#![cfg(feature = "pdfplumber")]
//! Diagnostic: why does pdfplumber extract invoice LINES but not CELLS?
//! Runs the full table-finder pipeline (edges -> snap -> join -> intersections
//! -> cells -> tables) and prints where it breaks, plus a tolerance sweep.
//! Run: cargo test --features pdfplumber --test pdfplumber_cell_debug_test -- --nocapture --ignored

use pdfplumber::{EdgeSource, Orientation, Pdf, TableFinder, TableSettings, WordOptions};
use std::path::Path;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

fn data_path(relative: &str) -> String {
    let normalized = relative.replace('/', "\\");
    format!("{DATA_DIR}\\{normalized}")
}

fn source_name(s: EdgeSource) -> &'static str {
    match s {
        EdgeSource::Line => "Line",
        EdgeSource::RectTop => "RectTop",
        EdgeSource::RectBottom => "RectBot",
        EdgeSource::RectLeft => "RectLeft",
        EdgeSource::RectRight => "RectRight",
        EdgeSource::Curve => "Curve",
        EdgeSource::Stream => "Stream",
        EdgeSource::Explicit => "Explicit",
    }
}

/// Manual intersection count at a given tolerance (independent of the library
/// function, so we can see whether tolerance is the culprit).
fn manual_cross_count(edges: &[pdfplumber::Edge], x_tol: f64, y_tol: f64) -> usize {
    let hs: Vec<_> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    let vs: Vec<_> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();
    let mut n = 0;
    for h in &hs {
        let y = h.top;
        for v in &vs {
            let x = v.x0;
            if x >= h.x0 - x_tol && x <= h.x1 + x_tol && y >= v.top - y_tol && y <= v.bottom + y_tol
            {
                n += 1;
            }
        }
    }
    n
}

fn diagnose_page(page: &pdfplumber::Page, pn: usize) {
    let lines = page.lines();
    let rects = page.rects();
    let curves = page.curves();
    eprintln!("\n========== PAGE {pn} ==========");
    eprintln!(
        "geometry: lines={} rects={} curves={}  (page {}x{})",
        lines.len(),
        rects.len(),
        curves.len(),
        page.width(),
        page.height()
    );

    // How are the raw lines oriented? (orientation comes from shape extraction)
    let mut line_orient = [0usize; 3]; // H, V, D
    for l in lines {
        line_orient[match l.orientation {
            Orientation::Horizontal => 0,
            Orientation::Vertical => 1,
            _ => 2,
        }] += 1;
    }
    eprintln!(
        "line orientation: H={} V={} Diagonal={}",
        line_orient[0], line_orient[1], line_orient[2]
    );

    // Classify rects into "bars": thin filled rects that ARE the grid lines.
    // A horizontal bar = wide & short (w >> h); a vertical bar = tall & thin (h >> w).
    let mut h_bars: Vec<(f64, f64, f64)> = Vec::new(); // (y_center, x0, x1)
    let mut v_bars: Vec<(f64, f64, f64)> = Vec::new(); // (x_center, y0, y1)
    let mut cell_rects: Vec<(f64, f64, f64, f64)> = Vec::new(); // genuine cell-sized rects
    for r in rects {
        let w = r.x1 - r.x0;
        let h = r.bottom - r.top;
        let thin = (w.min(h)) < 2.0; // bar thickness < 2pt
        if thin && w > h * 3.0 {
            h_bars.push(((r.top + r.bottom) / 2.0, r.x0, r.x1));
        } else if thin && h > w * 3.0 {
            v_bars.push(((r.x0 + r.x1) / 2.0, r.top, r.bottom));
        } else {
            cell_rects.push((r.x0, r.top, r.x1, r.bottom));
        }
    }
    h_bars.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    v_bars.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    eprintln!(
        "rects classified: H-bars={} V-bars={} cell-like={}",
        h_bars.len(),
        v_bars.len(),
        cell_rects.len()
    );
    eprintln!("  H-bars (y, x0..x1):");
    for (y, x0, x1) in h_bars.iter().take(30) {
        eprintln!(
            "    y={y:>8.2}  x:[{x0:>8.2} .. {x1:>8.2}]  len={:.2}",
            x1 - x0
        );
    }
    eprintln!("  V-bars (x, y0..y1):");
    for (x, y0, y1) in v_bars.iter().take(30) {
        eprintln!(
            "    x={x:>8.2}  y:[{y0:>8.2} .. {y1:>8.2}]  len={:.2}",
            y1 - y0
        );
    }

    // Raw edges (derive_edges from lines+rects+curves)
    let raw_edges = page.edges();
    let mut by_orient = [0usize; 3];
    let mut by_source = std::collections::HashMap::<&str, (usize, [usize; 3])>::new();
    for e in &raw_edges {
        let oi = match e.orientation {
            Orientation::Horizontal => 0,
            Orientation::Vertical => 1,
            _ => 2,
        };
        by_orient[oi] += 1;
        let s = source_name(e.source);
        let entry = by_source.entry(s).or_insert((0, [0; 3]));
        entry.0 += 1;
        entry.1[oi] += 1;
    }
    eprintln!(
        "raw edges: total={} H={} V={} Diagonal={}",
        raw_edges.len(),
        by_orient[0],
        by_orient[1],
        by_orient[2]
    );
    for (s, (n, o)) in &by_source {
        eprintln!(
            "  source {s:>9}: {n:>4}  (H={} V={} D={})",
            o[0], o[1], o[2]
        );
    }

    // Sample horizontal & vertical edges (after derive, before pipeline)
    let hs: Vec<_> = raw_edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    let vs: Vec<_> = raw_edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();
    eprintln!("--- sample H edges (x0, top=y, x1, bottom) ---");
    for e in hs.iter().take(12) {
        eprintln!(
            "  H y={:.2}  x:[{:.2} .. {:.2}]  src={}  len={:.2}",
            e.top,
            e.x0,
            e.x1,
            source_name(e.source),
            (e.x1 - e.x0).abs()
        );
    }
    eprintln!("--- sample V edges (x0, top, x1, bottom=y) ---");
    for e in vs.iter().take(12) {
        eprintln!(
            "  V x={:.2}  y:[{:.2} .. {:.2}]  src={}  len={:.2}",
            e.x0,
            e.top,
            e.bottom,
            source_name(e.source),
            (e.bottom - e.top).abs()
        );
    }

    // Run the real pipeline with DEFAULT settings
    let words = page.extract_words(&WordOptions::default());
    let finder = TableFinder::new_with_words(raw_edges.clone(), words, TableSettings::default());
    let debug = finder.find_tables_debug();
    eprintln!(
        "PIPELINE(default): edges(after)={} intersections={} cells={} tables={}",
        debug.edges.len(),
        debug.intersections.len(),
        debug.cells.len(),
        debug.tables.len()
    );

    // Always dump the intersection grid (unique xs / ys) and post-pipeline edges
    {
        let mut xs: Vec<f64> = debug.intersections.iter().map(|i| i.x).collect();
        let mut ys: Vec<f64> = debug.intersections.iter().map(|i| i.y).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        ys.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        eprintln!(
            "  intersection grid: {} unique x, {} unique y",
            xs.len(),
            ys.len()
        );
        eprintln!("    xs: {:?}", xs);
        eprintln!("    ys: {:?}", ys);
        // Dump post-pipeline H and V edges (these are what actually cross)
        let ph: Vec<_> = debug
            .edges
            .iter()
            .filter(|e| e.orientation == Orientation::Horizontal)
            .collect();
        let pv: Vec<_> = debug
            .edges
            .iter()
            .filter(|e| e.orientation == Orientation::Vertical)
            .collect();
        eprintln!("  post-pipeline H edges ({}):", ph.len());
        for e in ph.iter().take(20) {
            eprintln!(
                "    H y={:.2} x:[{:.2}..{:.2}] src={}",
                e.top,
                e.x0,
                e.x1,
                source_name(e.source)
            );
        }
        eprintln!("  post-pipeline V edges ({}):", pv.len());
        for e in pv.iter().take(20) {
            eprintln!(
                "    V x={:.2} y:[{:.2}..{:.2}] src={}",
                e.x0,
                e.top,
                e.bottom,
                source_name(e.source)
            );
        }
    }

    // Where does it break?
    if by_orient[0] > 0 && by_orient[1] > 0 && debug.intersections.is_empty() {
        eprintln!("!! BREAK at edges_to_intersections: H and V edges exist but 0 intersections");
        // Manual cross-check at increasing tolerances
        for tol in [0.0_f64, 1.0, 3.0, 6.0, 12.0, 24.0, 48.0] {
            let n = manual_cross_count(&raw_edges, tol, tol);
            eprintln!("   manual cross tol={tol:>5}: {n} candidate intersections (raw edges, pre-snap/join)");
        }
        // Also check on the post-snap/join edges
        for tol in [0.0_f64, 1.0, 3.0, 6.0, 12.0, 24.0, 48.0] {
            let n = manual_cross_count(&debug.edges, tol, tol);
            eprintln!(
                "   manual cross tol={tol:>5}: {n} (post-snap/join edges, {} edges)",
                debug.edges.len()
            );
        }
    } else if !debug.intersections.is_empty() && debug.cells.is_empty() {
        eprintln!(
            "!! BREAK at intersections_to_cells: intersections exist but 0 cells (missing corners)"
        );
        eprintln!("   unique x/y in intersections:");
        let mut xs: Vec<f64> = debug.intersections.iter().map(|i| i.x).collect();
        let mut ys: Vec<f64> = debug.intersections.iter().map(|i| i.y).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        ys.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        eprintln!("     xs({}): {:?}", xs.len(), xs);
        eprintln!("     ys({}): {:?}", ys.len(), ys);
    } else if !debug.cells.is_empty() && debug.tables.is_empty() {
        eprintln!("!! BREAK at cells_to_tables: cells exist but 0 tables");
    } else if !debug.tables.is_empty() {
        eprintln!(
            "OK: tables detected ({}). dumping ALL tables:",
            debug.tables.len()
        );
        for (ti, t) in debug.tables.iter().enumerate() {
            eprintln!(
                "  TABLE {ti}: bbox=({:.1},{:.1},{:.1},{:.1}) cells={} rows={}",
                t.bbox.x0,
                t.bbox.top,
                t.bbox.x1,
                t.bbox.bottom,
                t.cells.len(),
                t.rows.len()
            );
            for (ri, row) in t.rows.iter().take(8).enumerate() {
                let texts: Vec<String> = row
                    .iter()
                    .map(|c| c.text.clone().unwrap_or_default().replace('\n', " "))
                    .collect();
                let bxs: Vec<String> = row
                    .iter()
                    .map(|c| format!("({:.0},{:.0})", c.bbox.x0, c.bbox.top))
                    .collect();
                eprintln!("    row{ri} texts: {:?}", texts);
                eprintln!("    row{ri} bboxes: {}", bxs.join(" "));
            }
        }
    }

    // Tolerance sweep — which setting produces cells?
    eprintln!("--- tolerance sweep (cells / tables) ---");
    let base = TableSettings::default();
    let configs: Vec<(&str, TableSettings)> = vec![
        ("default", base.clone()),
        (
            "snap=0",
            TableSettings {
                snap_tolerance: 0.0,
                snap_x_tolerance: 0.0,
                snap_y_tolerance: 0.0,
                ..base.clone()
            },
        ),
        (
            "intersect=6",
            TableSettings {
                intersection_tolerance: 6.0,
                intersection_x_tolerance: 6.0,
                intersection_y_tolerance: 6.0,
                ..base.clone()
            },
        ),
        (
            "intersect=12",
            TableSettings {
                intersection_tolerance: 12.0,
                intersection_x_tolerance: 12.0,
                intersection_y_tolerance: 12.0,
                ..base.clone()
            },
        ),
        (
            "intersect=24",
            TableSettings {
                intersection_tolerance: 24.0,
                intersection_x_tolerance: 24.0,
                intersection_y_tolerance: 24.0,
                ..base.clone()
            },
        ),
        (
            "join=12",
            TableSettings {
                join_tolerance: 12.0,
                join_x_tolerance: 12.0,
                join_y_tolerance: 12.0,
                ..base.clone()
            },
        ),
        (
            "minlen=0",
            TableSettings {
                edge_min_length: 0.0,
                ..base.clone()
            },
        ),
        (
            "snap=0+intersect=12",
            TableSettings {
                snap_tolerance: 0.0,
                snap_x_tolerance: 0.0,
                snap_y_tolerance: 0.0,
                intersection_tolerance: 12.0,
                intersection_x_tolerance: 12.0,
                intersection_y_tolerance: 12.0,
                ..base.clone()
            },
        ),
        (
            "strict(lines only)",
            TableSettings {
                strategy: pdfplumber::Strategy::LatticeStrict,
                ..base.clone()
            },
        ),
    ];
    for (name, cfg) in &configs {
        let words = page.extract_words(&WordOptions::default());
        let finder = TableFinder::new_with_words(page.edges(), words, cfg.clone());
        let d = finder.find_tables_debug();
        eprintln!(
            "  {name:<22}: edges={} inter={} cells={} tables={}",
            d.edges.len(),
            d.intersections.len(),
            d.cells.len(),
            d.tables.len()
        );
    }
}

#[test]
fn diagnose_didi_invoice_a() {
    let pdf_path = data_path("市内交通\\滴滴电子发票A.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: {pdf_path} not found");
        return;
    }
    run_diag(&pdf_path);
}

#[test]
fn diagnose_vat_invoice() {
    let pdf_path = data_path(
        "住宿\\dzfp_26512000001728418261_中国人民解放军国防科技大学系统工程学院_20260427084626.pdf",
    );
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: {pdf_path} not found");
        return;
    }
    run_diag(&pdf_path);
}

#[test]
fn diagnose_tianfu_itinerary() {
    let pdf_path = data_path("行程单\\天府通\\天府通电子行程单.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: {pdf_path} not found");
        return;
    }
    run_diag(&pdf_path);
}

#[test]
fn verify_find_tables_text_population() {
    // End-to-end: page.find_tables() (unlike find_tables_debug) populates cell text.
    // Confirms the fixed pipeline yields a readable 7-column itinerary table.
    let pdf_path = data_path("行程单\\天府通\\天府通电子行程单.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: {pdf_path} not found");
        return;
    }
    let pdf = Pdf::open_file(&pdf_path, None).expect("open");
    let mut found_7col = false;
    for page_result in pdf.pages_iter() {
        let page = match page_result {
            Ok(p) => p,
            Err(_) => continue,
        };
        let tables = page.find_tables(&TableSettings::default());
        for t in &tables {
            // Look for a row with >= 6 cells (the 7-column itinerary body)
            for row in &t.rows {
                if row.len() >= 6 {
                    found_7col = true;
                    let texts: Vec<String> = row
                        .iter()
                        .map(|c| c.text.clone().unwrap_or_default().replace('\n', " "))
                        .collect();
                    eprintln!("7-col row: {:?}", texts);
                }
            }
        }
    }
    assert!(
        found_7col,
        "expected at least one >=6-column row in 天府通 table"
    );
}

#[test]
fn diagnose_didi_itinerary_a() {
    let pdf_path = data_path("行程单\\滴滴\\滴滴出行行程报销单 A.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: {pdf_path} not found");
        return;
    }
    run_diag(&pdf_path);
}

#[test]
fn diagnose_didi_itinerary_b() {
    let pdf_path = data_path("行程单\\滴滴\\滴滴出行行程报销单 B.pdf");
    if !Path::new(&pdf_path).exists() {
        eprintln!("SKIP: {pdf_path} not found");
        return;
    }
    run_diag(&pdf_path);
}

fn run_diag(pdf_path: &str) {
    eprintln!("\n\n########## DIAGNOSE: {pdf_path} ##########");
    let pdf = match Pdf::open_file(pdf_path, None) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("open_file failed: {e}; trying repair");
            let bytes = std::fs::read(pdf_path).expect("read");
            match Pdf::open_with_repair(&bytes, None, None) {
                Ok((p, _)) => p,
                Err(e2) => {
                    eprintln!("repair also failed: {e2}");
                    return;
                }
            }
        }
    };
    for page_result in pdf.pages_iter() {
        let page = match page_result {
            Ok(p) => p,
            Err(e) => {
                eprintln!("page error: {e}");
                continue;
            }
        };
        diagnose_page(&page, page.page_number());
    }
}
