use regex::Regex;
use std::sync::OnceLock;

/// 有序 datetime 格式（越靠前越具体，命中即返回）。每种格式的捕获组结构见 formatter。
#[derive(Clone, Copy)]
enum Kind {
    FullSec,     // YYYY-MM-DD HH:MM:SS
    Full,        // YYYY-MM-DD HH:MM（日期时间可粘连）
    Date,        // YYYY[-/年]MM[-/月]DD
    Short,       // MM-DD HH:MM（组件间容忍 \s*，含冒号后换行）
    ShortMergedMin, // MM-DD HH MM（无冒号，空格分隔分钟）
    ShortIncomplete, // MM-DD HH:??（分钟缺失哨兵）
    ShortDate,   // MM-DD
}

struct Pat {
    re: &'static str,
    kind: Kind,
}

static PATTERNS: &[Pat] = &[
    Pat { re: r"(\d{4})[-/年](\d{1,2})[-/月](\d{1,2})[\s日]*(\d{1,2}):(\d{2}):(\d{2})", kind: Kind::FullSec },
    Pat { re: r"(\d{4})[-/年](\d{1,2})[-/月](\d{1,2})[\s日]*(\d{1,2}):(\d{2})", kind: Kind::Full },
    Pat { re: r"(\d{4})[-/年](\d{1,2})[-/月](\d{1,2})", kind: Kind::Date },
    Pat { re: r"(\d{2})-(\d{2})[\s]*(\d{1,2})[:：][\s]*(\d{2})", kind: Kind::Short },
    Pat { re: r"(\d{2})-(\d{2})(\d{1,2})[\s]+(\d{2})", kind: Kind::ShortMergedMin },
    Pat { re: r"(\d{2})-(\d{2})(\d{1,2})", kind: Kind::ShortIncomplete },
    Pat { re: r"(\d{2})-(\d{2})[\s]*(\d{1,2})[:：]", kind: Kind::ShortIncomplete },
    Pat { re: r"(\d{2})-(\d{2})", kind: Kind::ShortDate },
];

fn compiled() -> &'static Vec<(Regex, Kind)> {
    static RE: OnceLock<Vec<(Regex, Kind)>> = OnceLock::new();
    RE.get_or_init(|| PATTERNS.iter().map(|p| (Regex::new(p.re).unwrap(), p.kind)).collect())
}

fn pad2(s: &str) -> String {
    s.parse::<u32>().map_or_else(|_| s.to_string(), |n| format!("{n:02}"))
}

/// Layer 1：从噪杂文本提取规范化 datetime 字符串。
/// 周几、换行、序号等噪声天然被正则忽略（只捕获数字部分），无需先剥周几。
/// 输出形态：`YYYY-MM-DD HH:MM:SS` / `YYYY-MM-DD HH:MM` / `YYYY-MM-DD` /
///          `MM-DD HH:MM` / `MM-DD HH:??` / `MM-DD`
pub fn extract_datetime(text: &str) -> Option<String> {
    for (re, kind) in compiled() {
        if let Some(c) = re.captures(text) {
            return Some(match kind {
                Kind::FullSec => format!("{}-{}-{} {}:{}:{}", &c[1], pad2(&c[2]), pad2(&c[3]), pad2(&c[4]), &c[5], &c[6]),
                Kind::Full => format!("{}-{}-{} {}:{}", &c[1], pad2(&c[2]), pad2(&c[3]), pad2(&c[4]), &c[5]),
                Kind::Date => format!("{}-{}-{}", &c[1], pad2(&c[2]), pad2(&c[3])),
                Kind::Short => format!("{}-{} {}:{}", &c[1], &c[2], pad2(&c[3]), &c[4]),
                Kind::ShortMergedMin => format!("{}-{} {}:{}", &c[1], &c[2], pad2(&c[3]), pad2(&c[4])),
                Kind::ShortIncomplete => format!("{}-{} {}:??", &c[1], &c[2], pad2(&c[3])),
                Kind::ShortDate => format!("{}-{}", &c[1], &c[2]),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_full_datetime() {
        assert_eq!(extract_datetime("2026-04-24 17:58:59").as_deref(), Some("2026-04-24 17:58:59"));
    }

    #[test]
    fn test_extract_full_no_space() {
        assert_eq!(extract_datetime("2026-04-2408:48:00").as_deref(), Some("2026-04-24 08:48:00"));
    }

    #[test]
    fn test_extract_full_no_seconds() {
        assert_eq!(extract_datetime("2026-04-24 08:48").as_deref(), Some("2026-04-24 08:48"));
    }

    #[test]
    fn test_extract_date_only_chinese_and_slash() {
        assert_eq!(extract_datetime("2026/04/24").as_deref(), Some("2026-04-24"));
        assert_eq!(extract_datetime("2026年04月24日").as_deref(), Some("2026-04-24"));
    }

    #[test]
    fn test_extract_short_weekday_split_newline() {
        assert_eq!(extract_datetime("05-11 11:48 周\n一").as_deref(), Some("05-11 11:48"));
    }

    #[test]
    fn test_extract_short_colon_newline_minutes() {
        assert_eq!(extract_datetime("04-22 21:\n10 周三").as_deref(), Some("04-22 21:10"));
    }

    #[test]
    fn test_extract_short_colon_space() {
        assert_eq!(extract_datetime("04-22 21: 10").as_deref(), Some("04-22 21:10"));
    }

    #[test]
    fn test_extract_short_incomplete_minutes() {
        assert_eq!(extract_datetime("07-03 20:").as_deref(), Some("07-03 20:??"));
    }

    #[test]
    fn test_extract_short_merged_incomplete() {
        assert_eq!(extract_datetime("07-0320").as_deref(), Some("07-03 20:??"));
    }

    #[test]
    fn test_extract_short_merged_minutes() {
        assert_eq!(extract_datetime("07-0320 46").as_deref(), Some("07-03 20:46"));
    }

    #[test]
    fn test_extract_short_date_only() {
        assert_eq!(extract_datetime("04-28").as_deref(), Some("04-28"));
    }

    #[test]
    fn test_extract_no_datetime_returns_none() {
        assert_eq!(extract_datetime("专车 成都"), None);
    }
}
