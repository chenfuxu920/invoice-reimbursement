//! 打印 PDF 中所有字体相关对象，用于诊断浏览器兼容性。
//! 用法: cargo run --bin dump_fonts <pdf路径>

use lopdf::Object;

fn main() {
    let path = std::env::args().nth(1).expect("用法: dump_fonts <pdf>");
    let doc = lopdf::Document::load(&path).expect("加载失败");

    for (id, obj) in &doc.objects {
        let is_font_dict = match obj {
            Object::Dictionary(d) => d
                .get(b"Type")
                .and_then(|t| t.as_name())
                .map(|t| t == b"Font")
                .unwrap_or(false),
            _ => false,
        };
        if !is_font_dict {
            continue;
        }
        let d = obj.as_dict().unwrap();
        println!("--- obj ({}, {}) ---", id.0, id.1);
        println!("{d:#?}");
    }
}
