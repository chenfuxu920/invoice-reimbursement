use invoice_reimbursement_lib::pdf::image_embedder::render_pdf_all_pages_to_pngs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pdf_path = args.get(1).expect("usage: dump_render_preview <pdf_path> [dpi]");
    let dpi: u32 = args.get(2).map(|s| s.parse().unwrap_or(150)).unwrap_or(150);
    let out_dir = "C:/Users/chenf/AppData/Local/Temp/opencode/preview_test";

    println!("rendering {} @ {}dpi → {}", pdf_path, dpi, out_dir);
    match render_pdf_all_pages_to_pngs(pdf_path, out_dir, dpi) {
        Ok(paths) => {
            for p in &paths {
                println!("  saved: {}", p.display());
            }
            println!("OK: {} pages", paths.len());
            // 用 explorer 打开输出目录
            std::process::Command::new("explorer")
                .arg(out_dir)
                .spawn()
                .ok();
        }
        Err(e) => {
            eprintln!("ERROR: {}", e);
            std::process::exit(1);
        }
    }
}
