//! 从 PDF 提取 FontFile2 嵌入字体到文件。
//! 用法: cargo run --bin extract_font <pdf路径> <输出ttf路径>

use lopdf::Object;

fn main() {
    let pdf = std::env::args().nth(1).expect("pdf路径");
    let out = std::env::args().nth(2).expect("输出路径");
    let doc = lopdf::Document::load(&pdf).expect("加载失败");

    // FontFile2 是带 Length1 键的流对象
    for (i, obj) in &doc.objects {
        let Object::Stream(s) = obj else { continue };
        if s.dict.get(b"Length1").is_err() {
            continue;
        }
        let mut s = s.clone();
        let _ = s.decompress();
        std::fs::write(&out, &s.content).expect("写出失败");
        println!("已提取 obj ({}, {}) → {out} ({} 字节)", i.0, i.1, s.content.len());
        return;
    }
    eprintln!("未找到 Length1 流");
    std::process::exit(1);
}
