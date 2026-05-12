use std::path::PathBuf;

fn main() {
    // 复制模型文件到输出目录（开发调试时需要）
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // OUT_DIR 是 target/debug/build/*/out，需要回退到 target/debug
    let target_debug = out_dir
        .ancestors()
        .find(|p| p.join("build").exists() && p.file_name().map_or(false, |n| n == "debug"))
        .unwrap_or(&out_dir)
        .to_path_buf();

    let models_src = PathBuf::from("models");
    let models_dst = target_debug.join("models");

    if models_src.exists() {
        // 创建目标目录
        std::fs::create_dir_all(&models_dst).ok();

        // 复制所有模型文件
        if let Ok(entries) = std::fs::read_dir(&models_src) {
            for entry in entries.flatten() {
                let src = entry.path();
                let dst = models_dst.join(entry.file_name());
                // 只在源文件更新时复制（避免每次编译都复制大文件）
                let should_copy = if dst.exists() {
                    let src_meta = std::fs::metadata(&src).ok();
                    let dst_meta = std::fs::metadata(&dst).ok();
                    match (src_meta, dst_meta) {
                        (Some(s), Some(d)) => s.len() != d.len() || s.modified().ok() != d.modified().ok(),
                        _ => true,
                    }
                } else {
                    true
                };
                if should_copy {
                    println!("cargo:warning=Copying model: {:?} -> {:?}", src, dst);
                    std::fs::copy(&src, &dst).ok();
                }
            }
        }
    }

    tauri_build::build()
}
