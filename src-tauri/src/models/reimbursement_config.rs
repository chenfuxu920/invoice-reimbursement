use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::AppHandle;
use tauri::Manager;

/// 扁平条目（region→standard），内部匹配用（激活集扁平化后的形式）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotelStandardEntry {
    pub region: String,    // 地区名（城市或省份），如 "成都市"、"四川省"
    pub standard: f64,     // 每晚上限（元）
}

/// 城市标准（挂在省份下）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CityStandard {
    pub name: String,          // 城市名，如 "福州"
    pub standard: f64,         // 每晚上限
}

/// 省份标准：default_standard 是"其他城市"的标准（未单独列出的城市用）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProvinceStandard {
    pub name: String,              // 省名，如 "福建省"
    pub default_standard: f64,     // 其他城市标准
    pub cities: Vec<CityStandard>, // 单独设置的城市
}

/// 一套住宿标准
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StandardSet {
    pub id: String,
    pub name: String,                  // 用户可改
    pub default_hotel_standard: f64,   // 该套未匹配任何省份时的兜底
    pub provinces: Vec<ProvinceStandard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReimbursementConfig {
    pub city_transport_daily: f64,
    pub meal_subsidy_daily: f64,
    pub active_standard_set_id: String, // "builtin" 或用户集 id
    pub standard_sets: Vec<StandardSet>,
    // 旧版字段：仅供反序列化迁移，不再序列化
    #[serde(default, skip_serializing)]
    pub default_hotel_standard: f64,
    #[serde(default, skip_serializing)]
    pub hotel_standards: Vec<HotelStandardEntry>,
}

impl Default for ReimbursementConfig {
    fn default() -> Self {
        Self {
            city_transport_daily: 80.0,
            meal_subsidy_daily: 100.0,
            active_standard_set_id: "builtin".to_string(),
            standard_sets: Vec::new(),
            default_hotel_standard: 350.0,
            hotel_standards: Vec::new(),
        }
    }
}

// 全局状态（进程内生效，启动/保存时由 apply 写入）
static ACTIVE_RULES: Mutex<Option<(Vec<HotelStandardEntry>, f64)>> = Mutex::new(None); // 激活用户集的扁平条目 + 集兜底
static CUSTOM_CITY_TRANSPORT_DAILY: Mutex<Option<f64>> = Mutex::new(None);
static CUSTOM_MEAL_SUBSIDY_DAILY: Mutex<Option<f64>> = Mutex::new(None);

/// 读取报销标准配置（文件不存在或解析失败则返回默认；旧版扁平配置自动迁移为用户集）
pub fn load_config(app: &AppHandle) -> ReimbursementConfig {
    let Some(dir) = app.path().app_data_dir().ok() else {
        return ReimbursementConfig::default();
    };
    let path = dir.join("reimbursement-config.json");
    let mut cfg: ReimbursementConfig = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    migrate_legacy(&mut cfg);
    cfg
}

/// 旧版扁平自定义（hotel_standards + default_hotel_standard）→ 迁移为一个用户集并激活
fn migrate_legacy(cfg: &mut ReimbursementConfig) {
    if !cfg.standard_sets.is_empty() || cfg.hotel_standards.is_empty() {
        return;
    }
    let provinces = crate::models::hotel_standard::build_province_structure(&cfg.hotel_standards);
    let default = if cfg.default_hotel_standard > 0.0 {
        cfg.default_hotel_standard
    } else {
        350.0
    };
    let set = StandardSet {
        id: uuid::Uuid::new_v4().to_string(),
        name: "我的标准".into(),
        default_hotel_standard: default,
        provinces,
    };
    cfg.active_standard_set_id = set.id.clone();
    cfg.standard_sets.push(set);
}

/// 保存报销标准配置
pub fn save_config(app: &AppHandle, config: &ReimbursementConfig) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("reimbursement-config.json"), json).map_err(|e| e.to_string())
}

/// 清理非法输入：金额不为负；剔除空名/重复 id/无效城市；无效激活 id 回退 builtin
pub fn sanitize(mut c: ReimbursementConfig) -> ReimbursementConfig {
    c.city_transport_daily = c.city_transport_daily.max(0.0);
    c.meal_subsidy_daily = c.meal_subsidy_daily.max(0.0);
    let mut used = std::collections::HashSet::new();
    for s in &mut c.standard_sets {
        if s.id.trim().is_empty() || !used.insert(s.id.clone()) {
            s.id = uuid::Uuid::new_v4().to_string();
        }
        s.name = {
            let t = s.name.trim();
            if t.is_empty() {
                "未命名标准".to_string()
            } else {
                t.to_string()
            }
        };
        s.default_hotel_standard = s.default_hotel_standard.max(0.0);
        s.provinces.retain(|p| !p.name.trim().is_empty());
        for p in &mut s.provinces {
            p.name = p.name.trim().to_string();
            p.default_standard = p.default_standard.max(0.0);
            p.cities.retain(|c| !c.name.trim().is_empty() && c.standard > 0.0);
            for c in &mut p.cities {
                c.name = c.name.trim().to_string();
                c.standard = c.standard.max(0.0);
            }
        }
    }
    if c.active_standard_set_id != "builtin"
        && !c.standard_sets.iter().any(|s| s.id == c.active_standard_set_id)
    {
        c.active_standard_set_id = "builtin".to_string();
    }
    c
}

/// 把配置写入进程内全局状态
// ponytail: 进程内全局生效（启动+保存时写入），避免改函数签名；测试/CLI 未 apply 时回退内置默认
pub fn apply_config(config: &ReimbursementConfig) {
    *CUSTOM_CITY_TRANSPORT_DAILY.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(config.city_transport_daily);
    *CUSTOM_MEAL_SUBSIDY_DAILY.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(config.meal_subsidy_daily);

    // active = "builtin" → 无激活用户集；否则找集扁平化，找不到（无效 id）→ None
    let active = if config.active_standard_set_id == "builtin" {
        None
    } else {
        config
            .standard_sets
            .iter()
            .find(|s| s.id == config.active_standard_set_id)
            .map(|set| (flatten_set(set), set.default_hotel_standard))
    };
    *ACTIVE_RULES.lock().unwrap_or_else(|e| e.into_inner()) = active;
}

/// 把一套标准集扁平化为条目：省份 → {region: 省名, standard: default_standard}；城市 → {region: 城市名, standard}
fn flatten_set(set: &StandardSet) -> Vec<HotelStandardEntry> {
    let mut entries = Vec::new();
    for p in &set.provinces {
        entries.push(HotelStandardEntry {
            region: p.name.clone(),
            standard: p.default_standard,
        });
        for c in &p.cities {
            entries.push(HotelStandardEntry {
                region: c.name.clone(),
                standard: c.standard,
            });
        }
    }
    entries
}

/// 当前激活用户集的扁平条目 + 集兜底（None = 未激活用户集，走内置标准）
pub fn active_set_rules() -> Option<(Vec<HotelStandardEntry>, f64)> {
    ACTIVE_RULES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

pub fn city_transport_daily() -> f64 {
    CUSTOM_CITY_TRANSPORT_DAILY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .unwrap_or(80.0)
}

pub fn meal_subsidy_daily() -> f64 {
    CUSTOM_MEAL_SUBSIDY_DAILY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .unwrap_or(100.0)
}

/// 在扁平条目中查找匹配：双向子串匹配，命中返回标准值
pub fn find_entry(rules: &[HotelStandardEntry], keyword: &str) -> Option<f64> {
    for r in rules {
        if keyword.contains(r.region.as_str()) || r.region.contains(keyword) {
            return Some(r.standard);
        }
    }
    None
}
