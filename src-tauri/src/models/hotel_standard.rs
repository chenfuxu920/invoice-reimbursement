/// 住宿标准管理
///
/// 根据目的地匹配住宿每晚上限标准。
/// - 城市→省份数据来源：github.com/modood/Administrative-divisions-of-China (342城市)
/// - 住宿标准数据来源：data/住宿标准.xlsx
///
/// 匹配逻辑：先匹配市，匹配不到再匹配省。
use calamine::{open_workbook, Reader, Xlsx};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
struct ProvinceEntry {
    code: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct CityEntry {
    #[allow(dead_code)]
    code: String,
    name: String,
    #[serde(rename = "provinceCode")]
    province_code: String,
}

/// 一条住宿标准规则
#[derive(Debug, Clone)]
struct HotelStandardRule {
    regions: Vec<String>,
    standard: f64,
}

static HOTEL_STANDARDS: OnceLock<Vec<HotelStandardRule>> = OnceLock::new();
static CITY_PROVINCE: OnceLock<HashMap<String, String>> = OnceLock::new();

/// 加载城市→省份映射（编译时嵌入 JSON）
fn city_province_map() -> &'static HashMap<String, String> {
    CITY_PROVINCE.get_or_init(|| {
        let mut m = HashMap::new();

        // 编译时嵌入的 JSON 数据
        let provinces_json = include_str!("../../../data/参考数据/provinces.json");
        let cities_json = include_str!("../../../data/参考数据/cities.json");

        let provinces: Vec<ProvinceEntry> =
            serde_json::from_str(provinces_json).unwrap_or_default();
        let cities: Vec<CityEntry> = serde_json::from_str(cities_json).unwrap_or_default();

        // 构建 code → 省名 映射
        let province_map: HashMap<&str, &str> = provinces
            .iter()
            .map(|p| (p.code.as_str(), p.name.as_str()))
            .collect();

        // 城市名 → 省名
        for city in &cities {
            if let Some(province_name) = province_map.get(city.province_code.as_str()) {
                // 存储完整城市名（如 "成都市"）和短名（如 "成都"）
                m.insert(city.name.clone(), province_name.to_string());
                let short_name = city
                    .name
                    .trim_end_matches('市')
                    .trim_end_matches('区')
                    .trim_end_matches('县');
                if short_name.len() >= 2 && short_name != city.name {
                    m.insert(short_name.to_string(), province_name.to_string());
                }
            }
        }

        // 直辖市：自己映射到自己
        for name in &["北京市", "天津市", "上海市", "重庆市"] {
            m.insert(name.to_string(), name.to_string());
        }

        m
    })
}

/// 加载并解析住宿标准表
fn load_standards() -> &'static Vec<HotelStandardRule> {
    HOTEL_STANDARDS.get_or_init(|| {
        let path = find_standard_file().unwrap_or_default();
        if path.is_empty() {
            return default_standards();
        }
        match parse_standards_xlsx(&path) {
            Ok(rules) if !rules.is_empty() => rules,
            _ => default_standards(),
        }
    })
}

fn find_standard_file() -> Option<String> {
    let candidates = [
        "../data/住宿标准.xlsx",
        "data/住宿标准.xlsx",
        "../../data/住宿标准.xlsx",
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

fn parse_standards_xlsx(path: &str) -> Result<Vec<HotelStandardRule>, Box<dyn std::error::Error>> {
    let mut workbook: Xlsx<_> = open_workbook(path)?;
    let mut rules = Vec::new();

    for sheet_name in workbook.sheet_names().to_owned() {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            for row in range.rows() {
                if row.len() < 2 {
                    continue;
                }
                let region_str = match &row[0] {
                    calamine::Data::String(s) => s.trim().to_string(),
                    _ => continue,
                };
                if region_str.is_empty() {
                    continue;
                }
                let standard: f64 = match &row[1] {
                    calamine::Data::Float(f) => *f,
                    calamine::Data::String(s) => s.trim().parse().unwrap_or(0.0),
                    _ => 0.0,
                };
                if standard <= 0.0 {
                    continue;
                }
                let regions: Vec<String> = region_str
                    .split('、')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                rules.push(HotelStandardRule { regions, standard });
            }
        }
    }
    Ok(rules)
}

fn default_standards() -> Vec<HotelStandardRule> {
    vec![
        HotelStandardRule {
            regions: vec!["北京市".to_string()],
            standard: 500.0,
        },
        HotelStandardRule {
            regions: vec!["上海市".to_string()],
            standard: 500.0,
        },
        HotelStandardRule {
            regions: vec!["广东省".to_string(), "深圳市".to_string()],
            standard: 450.0,
        },
        HotelStandardRule {
            regions: vec!["浙江省".to_string(), "厦门市".to_string()],
            standard: 400.0,
        },
        HotelStandardRule {
            regions: vec!["江苏省".to_string()],
            standard: 380.0,
        },
        HotelStandardRule {
            regions: vec![
                "福建省".to_string(),
                "河南省".to_string(),
                "云南省".to_string(),
            ],
            standard: 380.0,
        },
        HotelStandardRule {
            regions: vec!["四川省".to_string()],
            standard: 370.0,
        },
        HotelStandardRule {
            regions: vec!["重庆市".to_string()],
            standard: 370.0,
        },
        HotelStandardRule {
            regions: vec!["贵州省".to_string()],
            standard: 370.0,
        },
        HotelStandardRule {
            regions: vec![
                "山东省".to_string(),
                "天津市".to_string(),
                "青岛市".to_string(),
            ],
            standard: 380.0,
        },
        HotelStandardRule {
            regions: vec![
                "青海省".to_string(),
                "海南省".to_string(),
                "西藏自治区".to_string(),
            ],
            standard: 350.0,
        },
        HotelStandardRule {
            regions: vec!["大连市".to_string()],
            standard: 350.0,
        },
        HotelStandardRule {
            regions: vec![
                "山西省".to_string(),
                "湖北省".to_string(),
                "辽宁省".to_string(),
                "新疆维吾尔自治区".to_string(),
            ],
            standard: 350.0,
        },
        HotelStandardRule {
            regions: vec![
                "江西省".to_string(),
                "甘肃省".to_string(),
                "广西壮族自治区".to_string(),
                "宁夏回族自治区".to_string(),
            ],
            standard: 350.0,
        },
        HotelStandardRule {
            regions: vec![
                "安徽省".to_string(),
                "陕西省".to_string(),
                "内蒙古自治区".to_string(),
            ],
            standard: 350.0,
        },
        HotelStandardRule {
            regions: vec![
                "河北省".to_string(),
                "吉林省".to_string(),
                "湖南省".to_string(),
                "黑龙江省".to_string(),
            ],
            standard: 350.0,
        },
    ]
}

/// 在规则列表中查找匹配
fn find_in_rules<'a>(rules: &'a [HotelStandardRule], keyword: &str) -> Option<f64> {
    for rule in rules {
        for region in &rule.regions {
            if keyword.contains(region.as_str()) || region.contains(keyword) {
                return Some(rule.standard);
            }
        }
    }
    None
}

/// 从目的地提取城市关键词（去掉"市/区/县"后缀）
fn extract_city_keyword(dest: &str) -> Vec<String> {
    let mut keywords = Vec::new();
    let chars: Vec<char> = dest.chars().collect();

    // 尝试去掉常见后缀
    for suffix in &["市", "区", "县", "自治州", "地区"] {
        if let Some(pos) = dest.find(suffix) {
            let before = &dest[..pos];
            let before_chars: Vec<char> = before.chars().collect();
            // 取最后 2-4 个字符
            for len in &[4usize, 3, 2] {
                if before_chars.len() >= *len {
                    let start = before_chars.len() - len;
                    let kw: String = before_chars[start..].iter().collect();
                    if kw.len() >= 2 {
                        keywords.push(kw);
                    }
                }
            }
        }
    }

    // 如果没有后缀，直接用完整字符串尝试
    if keywords.is_empty() && chars.len() >= 2 {
        keywords.push(dest.to_string());
    }

    keywords
}

/// 由内置扁平标准（xlsx/默认表）派生省份层级结构（供设置页展示默认标准 & 前端复制）
pub fn get_builtin_hotel_standards() -> Vec<crate::models::reimbursement_config::ProvinceStandard> {
    build_province_structure(&builtin_flat_entries())
}

/// 内置扁平条目（每个 region 展开为独立条目）
fn builtin_flat_entries() -> Vec<crate::models::reimbursement_config::HotelStandardEntry> {
    load_standards()
        .iter()
        .flat_map(|r| {
            r.regions.iter().map(
                |region| crate::models::reimbursement_config::HotelStandardEntry {
                    region: region.clone(),
                    standard: r.standard,
                },
            )
        })
        .collect()
}

/// 把扁平条目（region→standard）派生成省份→城市层级。
/// 供内置标准展示和旧配置迁移共用。
pub fn build_province_structure(
    entries: &[crate::models::reimbursement_config::HotelStandardEntry],
) -> Vec<crate::models::reimbursement_config::ProvinceStandard> {
    use crate::models::reimbursement_config::{CityStandard, ProvinceStandard};
    use std::collections::{HashMap, HashSet};
    let map = city_province_map();
    let provinces: HashSet<String> = map.values().cloned().collect();
    let mut nodes: HashMap<String, ProvinceStandard> = HashMap::new();
    for e in entries {
        let region = e.region.trim().to_string();
        if region.is_empty() {
            continue;
        }
        if provinces.contains(&region) {
            // 省（含直辖市）→ 省份节点，"其他城市"标准
            nodes
                .entry(region.clone())
                .or_insert_with(|| ProvinceStandard {
                    name: region.clone(),
                    default_standard: 350.0,
                    cities: vec![],
                })
                .default_standard = e.standard;
        } else if let Some(province) = map.get(&region) {
            // 城市 → 挂到其省份下
            let node = nodes
                .entry(province.clone())
                .or_insert_with(|| ProvinceStandard {
                    name: province.clone(),
                    default_standard: 350.0,
                    cities: vec![],
                });
            node.cities.push(CityStandard {
                name: region,
                standard: e.standard,
            });
        } else {
            // 未知地区 → 自成一省节点（不丢数据）
            nodes
                .entry(region.clone())
                .or_insert_with(|| ProvinceStandard {
                    name: region.clone(),
                    default_standard: e.standard,
                    cities: vec![],
                });
        }
    }
    let mut list: Vec<ProvinceStandard> = nodes.into_values().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    for n in &mut list {
        n.cities.sort_by(|a, b| a.name.cmp(&b.name));
    }
    list
}

/// 获取指定目的地的住宿每晚上限标准（元/晚）
///
/// 匹配逻辑：
/// - 激活了用户标准集 → 只用用户集（直接匹配 → 城市→省份 → 集兜底）
/// - 否则用内置标准（直接匹配 xlsx/默认表 → 城市→省份 → 350）
pub fn get_hotel_nightly_rate_std(destination: &str) -> f64 {
    if destination.is_empty() {
        return crate::models::reimbursement_config::active_set_rules()
            .map(|(_, d)| d)
            .unwrap_or(350.0);
    }
    let dest = destination.trim();
    let rules = load_standards();
    let city_map = city_province_map();

    // 激活了用户标准集 → 只用用户集（直接匹配 → 城市→省份 → 集兜底）
    if let Some((entries, set_default)) = crate::models::reimbursement_config::active_set_rules() {
        if let Some(std) = crate::models::reimbursement_config::find_entry(&entries, dest) {
            return std;
        }
        let keywords = extract_city_keyword(dest);
        for kw in &keywords {
            if let Some(province) = city_map.get(kw) {
                if let Some(std) =
                    crate::models::reimbursement_config::find_entry(&entries, province)
                {
                    return std;
                }
            }
        }
        return set_default;
    }

    // 内置标准（原逻辑，兜底 350）
    if let Some(std) = find_in_rules(rules, dest) {
        return std;
    }
    let keywords = extract_city_keyword(dest);
    for kw in &keywords {
        if let Some(province) = city_map.get(kw) {
            if let Some(std) = find_in_rules(rules, province) {
                return std;
            }
        }
    }
    350.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_city_in_xlsx() {
        assert_eq!(get_hotel_nightly_rate_std("深圳市"), 450.0);
        assert_eq!(get_hotel_nightly_rate_std("深圳市南山区"), 450.0);
        assert_eq!(get_hotel_nightly_rate_std("青岛市"), 380.0);
        assert_eq!(get_hotel_nightly_rate_std("大连市"), 350.0);
        assert_eq!(get_hotel_nightly_rate_std("厦门市"), 400.0);
    }

    #[test]
    fn test_match_city_via_province() {
        assert_eq!(get_hotel_nightly_rate_std("成都市"), 370.0);
        assert_eq!(get_hotel_nightly_rate_std("广州市"), 450.0);
        assert_eq!(get_hotel_nightly_rate_std("杭州市"), 400.0);
        assert_eq!(get_hotel_nightly_rate_std("南京市"), 380.0);
        assert_eq!(get_hotel_nightly_rate_std("武汉市"), 350.0);
        assert_eq!(get_hotel_nightly_rate_std("长沙市"), 350.0);
        assert_eq!(get_hotel_nightly_rate_std("郑州市"), 380.0);
        assert_eq!(get_hotel_nightly_rate_std("昆明市"), 380.0);
        assert_eq!(get_hotel_nightly_rate_std("贵阳市"), 370.0);
        assert_eq!(get_hotel_nightly_rate_std("西安市"), 350.0);
        assert_eq!(get_hotel_nightly_rate_std("合肥市"), 350.0);
    }

    #[test]
    fn test_match_province_directly() {
        assert_eq!(get_hotel_nightly_rate_std("四川省成都市"), 370.0);
        assert_eq!(get_hotel_nightly_rate_std("广东省广州市"), 450.0);
        assert_eq!(get_hotel_nightly_rate_std("浙江省杭州市"), 400.0);
        assert_eq!(get_hotel_nightly_rate_std("重庆市"), 370.0);
    }

    #[test]
    fn test_no_match_returns_default() {
        assert_eq!(get_hotel_nightly_rate_std(""), 350.0);
        assert_eq!(get_hotel_nightly_rate_std("未知地点"), 350.0);
    }

    #[test]
    fn test_comprehensive_city_coverage() {
        // 测试更多城市，验证开源数据覆盖
        assert_eq!(get_hotel_nightly_rate_std("拉萨市"), 350.0); // 西藏自治区
        assert_eq!(get_hotel_nightly_rate_std("银川市"), 350.0); // 宁夏回族自治区
        assert_eq!(get_hotel_nightly_rate_std("乌鲁木齐市"), 350.0); // 新疆维吾尔自治区
        assert_eq!(get_hotel_nightly_rate_std("兰州市"), 350.0); // 甘肃省
        assert_eq!(get_hotel_nightly_rate_std("南宁市"), 350.0); // 广西壮族自治区
        assert_eq!(get_hotel_nightly_rate_std("海口市"), 350.0); // 海南省
        assert_eq!(get_hotel_nightly_rate_std("西宁市"), 350.0); // 青海省
    }

    #[test]
    fn test_city_province_map_loaded() {
        let map = city_province_map();
        // 应该有 342+ 个城市映射
        assert!(map.len() > 300, "Expected 300+ mappings, got {}", map.len());
        // 验证一些关键映射
        assert_eq!(map.get("成都").unwrap(), "四川省");
        assert_eq!(map.get("广州").unwrap(), "广东省");
        assert_eq!(map.get("拉萨").unwrap(), "西藏自治区");
    }
}
