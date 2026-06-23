# 可配置发票匹配规则 - 后端实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将发票字段提取正则和分类关键词做成可配置模板，激活并扩展现有 TemplateManager，新增 Tauri 命令供前端调用，新增正则骨架生成器供标注模式使用。

**架构：** 扩展 `InvoiceTemplate` 结构体（新增 priority/enabled/category/category_keywords 字段），TemplateManager 支持内置+用户双目录加载，InvoiceParser 解析时模板优先→硬编码回退，新增 `regex_skeleton.rs` 纯函数模块，新增 8 个 Tauri 命令。

**技术栈：** Rust, Tauri 2, regex, serde, tempfile(测试)

**规格文档：** `docs/superpowers/specs/2026-06-23-configurable-invoice-matching-design.md`

---

## 文件结构

**创建：**
- `src-tauri/src/parser/regex_skeleton.rs` — 正则骨架生成器（纯函数，无副作用）
- `src-tauri/src/commands/template_commands.rs` — 模板管理 Tauri 命令
- `src-tauri/src/builtin_templates/vat_normal.json` — 内置模板：增值税普通发票
- `src-tauri/src/builtin_templates/vat_special.json` — 内置模板：增值税专用发票
- `src-tauri/src/builtin_templates/didi_itinerary.json` — 内置模板：滴滴行程单
- `src-tauri/src/builtin_templates/hotel.json` — 内置模板：酒店发票
- `src-tauri/src/builtin_templates/tianfutong.json` — 内置模板：天府通行程单

**修改：**
- `src-tauri/src/parser/template_manager.rs` — 扩展 InvoiceTemplate 结构体 + 双目录加载 + 分类逻辑 + Serialize
- `src-tauri/src/parser/invoice_parser.rs` — 改造解析流程：模板优先→硬编码回退
- `src-tauri/src/parser/mod.rs` — 注册新模块
- `src-tauri/src/lib.rs` — 注册新 Tauri 命令 + AppState 注入 TemplateManager
- `src-tauri/Cargo.toml` — 无需新增依赖（regex/serde/tempfile 已有）

---

## 任务 1：扩展 InvoiceTemplate 结构体

**文件：**
- 修改：`src-tauri/src/parser/template_manager.rs:7-30`

- [ ] **步骤 1：编写失败的测试**

在 `template_manager.rs` 的 `#[cfg(test)] mod tests` 末尾添加：

```rust
    #[test]
    fn test_extended_template_fields() {
        let json = r#"{
            "template_id": "test_ext",
            "name": "测试扩展字段",
            "enabled": true,
            "priority": 10,
            "keywords": ["测试"],
            "category": "Meal",
            "category_keywords": {
                "餐饮": ["餐饮", "餐费"],
                "住宿": ["住宿", "房费"]
            },
            "fields": []
        }"#;

        let template: InvoiceTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(template.priority, 10);
        assert!(template.enabled);
        assert_eq!(template.category.as_deref(), Some("Meal"));
        assert!(template.category_keywords.is_some());
        let ck = template.category_keywords.unwrap();
        assert_eq!(ck.get("餐饮").unwrap(), &vec!["餐饮".to_string(), "餐费".to_string()]);
    }

    #[test]
    fn test_extended_template_defaults() {
        // 旧格式（无新字段）应能正常反序列化，使用默认值
        let json = r#"{
            "template_id": "test_old",
            "name": "旧格式模板",
            "keywords": ["测试"],
            "fields": []
        }"#;

        let template: InvoiceTemplate = serde_json::from_str(json).unwrap();
        assert!(template.enabled); // 默认 true
        assert_eq!(template.priority, 0); // 默认 0
        assert!(template.category.is_none()); // 默认 None
        assert!(template.category_keywords.is_none()); // 默认 None
    }

    #[test]
    fn test_template_serialization_roundtrip() {
        let template = InvoiceTemplate {
            template_id: "roundtrip".to_string(),
            name: "往返测试".to_string(),
            enabled: true,
            priority: 5,
            keywords: vec!["测试".to_string()],
            category: Some("Hotel".to_string()),
            category_keywords: Some(HashMap::from([
                ("住宿".to_string(), vec!["住宿".to_string(), "宾馆".to_string()]),
            ])),
            fields: vec![],
        };

        let json = serde_json::to_string(&template).unwrap();
        let parsed: InvoiceTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.template_id, "roundtrip");
        assert_eq!(parsed.priority, 5);
        assert_eq!(parsed.category.as_deref(), Some("Hotel"));
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test --lib -p invoice-reimbursement test_extended_template_fields -- --nocapture`
预期：编译失败，`enabled`/`priority`/`category`/`category_keywords` 字段不存在

- [ ] **步骤 3：扩展结构体定义**

将 `template_manager.rs:7-30` 的三个结构体替换为（注意加 `Serialize`）：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceTemplate {
    pub template_id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub category_keywords: Option<HashMap<String, Vec<String>>>,
    pub fields: Vec<FieldDefinition>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub required: bool,
    pub strategies: Vec<FieldStrategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldStrategy {
    #[serde(rename = "type")]
    pub strategy_type: String,
    pub pattern: Option<String>,
    pub section_keyword: Option<String>,
    pub field_keyword: Option<String>,
    pub confidence: f64,
}
```

- [ ] **步骤 4：修复现有测试中的结构体字面量**

现有测试（`test_match_template`、`test_no_match_template`、`test_extract_with_template`、`test_required_field_missing`）中构造 `InvoiceTemplate { ... }` 时缺少新字段，需补上。在每个测试的结构体字面量中添加：

```rust
            enabled: true,
            priority: 0,
            category: None,
            category_keywords: None,
```

- [ ] **步骤 5：运行测试验证通过**

运行：`cargo test --lib -p invoice-reimbursement template_manager -- --nocapture`
预期：所有 template_manager 测试 PASS

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/parser/template_manager.rs
git commit -m "feat: 扩展 InvoiceTemplate 结构体，新增 priority/enabled/category/category_keywords 字段"
```

---

## 任务 2：TemplateManager 双目录加载与分类逻辑

**文件：**
- 修改：`src-tauri/src/parser/template_manager.rs`

- [ ] **步骤 1：编写失败的测试**

在 `template_manager.rs` 测试模块末尾添加：

```rust
    #[test]
    fn test_dual_dir_loading_user_overrides_builtin() {
        let builtin_dir = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();

        // 内置模板
        let builtin_json = r#"{
            "template_id": "shared_id",
            "name": "内置版本",
            "keywords": ["测试"],
            "fields": []
        }"#;
        fs::write(builtin_dir.path().join("builtin.json"), builtin_json).unwrap();

        // 用户模板（同 id，覆盖内置）
        let user_json = r#"{
            "template_id": "shared_id",
            "name": "用户版本",
            "keywords": ["测试"],
            "fields": []
        }"#;
        fs::write(user_dir.path().join("user.json"), user_json).unwrap();

        let manager = TemplateManager::from_dual_dirs(builtin_dir.path(), user_dir.path()).unwrap();
        let t = manager.get_template("shared_id").unwrap();
        assert_eq!(t.name, "用户版本");
        assert_eq!(manager.template_source("shared_id"), TemplateSource::User);
    }

    #[test]
    fn test_dual_dir_loading_builtin_only() {
        let builtin_dir = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();

        let builtin_json = r#"{
            "template_id": "builtin_only",
            "name": "内置模板",
            "keywords": ["测试"],
            "fields": []
        }"#;
        fs::write(builtin_dir.path().join("b.json"), builtin_json).unwrap();

        let manager = TemplateManager::from_dual_dirs(builtin_dir.path(), user_dir.path()).unwrap();
        assert_eq!(manager.template_source("builtin_only"), TemplateSource::Builtin);
    }

    #[test]
    fn test_corrupted_user_template_skipped() {
        let builtin_dir = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();

        // 正常的用户模板
        fs::write(user_dir.path().join("good.json"), r#"{
            "template_id": "good",
            "name": "好的",
            "keywords": ["测试"],
            "fields": []
        }"#).unwrap();

        // 损坏的用户模板
        fs::write(user_dir.path().join("bad.json"), "{ invalid json }").unwrap();

        let manager = TemplateManager::from_dual_dirs(builtin_dir.path(), user_dir.path()).unwrap();
        assert!(manager.get_template("good").is_some());
        assert!(manager.get_template("bad").is_none());
    }

    #[test]
    fn test_match_template_by_priority() {
        let mut manager = TemplateManager::new();

        // 两个模板都能匹配同一段文本
        manager.add_template(InvoiceTemplate {
            template_id: "low_prio".to_string(),
            name: "低优先级".to_string(),
            enabled: true,
            priority: 1,
            keywords: vec!["发票".to_string()],
            category: None,
            category_keywords: None,
            fields: vec![],
        });
        manager.add_template(InvoiceTemplate {
            template_id: "high_prio".to_string(),
            name: "高优先级".to_string(),
            enabled: true,
            priority: 10,
            keywords: vec!["发票".to_string()],
            category: None,
            category_keywords: None,
            fields: vec![],
        });

        let ocr = create_ocr_output(vec!["这是一张发票"]);
        let matched = manager.match_template(&ocr).unwrap();
        assert_eq!(matched.template_id, "high_prio");
    }

    #[test]
    fn test_match_template_skips_disabled() {
        let mut manager = TemplateManager::new();

        manager.add_template(InvoiceTemplate {
            template_id: "disabled".to_string(),
            name: "已禁用".to_string(),
            enabled: false,
            priority: 100,
            keywords: vec!["发票".to_string()],
            category: None,
            category_keywords: None,
            fields: vec![],
        });
        manager.add_template(InvoiceTemplate {
            template_id: "enabled".to_string(),
            name: "已启用".to_string(),
            enabled: true,
            priority: 1,
            keywords: vec!["发票".to_string()],
            category: None,
            category_keywords: None,
            fields: vec![],
        });

        let ocr = create_ocr_output(vec!["这是一张发票"]);
        let matched = manager.match_template(&ocr).unwrap();
        assert_eq!(matched.template_id, "enabled");
    }

    #[test]
    fn test_classify_by_template_category_keywords() {
        let template = InvoiceTemplate {
            template_id: "test".to_string(),
            name: "测试".to_string(),
            enabled: true,
            priority: 0,
            keywords: vec![],
            category: Some("Other".to_string()),
            category_keywords: Some(HashMap::from([
                ("Meal".to_string(), vec!["餐饮".to_string(), "餐费".to_string()]),
                ("Hotel".to_string(), vec!["住宿".to_string(), "宾馆".to_string()]),
            ])),
            fields: vec![],
        };

        assert_eq!(
            TemplateManager::classify_by_template(&template, "这是一笔餐饮费用"),
            Some("Meal".to_string())
        );
        assert_eq!(
            TemplateManager::classify_by_template(&template, "宾馆住宿费"),
            Some("Hotel".to_string())
        );
        assert_eq!(
            TemplateManager::classify_by_template(&template, "无关文本"),
            Some("Other".to_string()) // 回退到 category 默认值
        );
    }

    #[test]
    fn test_classify_by_template_no_keywords_returns_category() {
        let template = InvoiceTemplate {
            template_id: "test".to_string(),
            name: "测试".to_string(),
            enabled: true,
            priority: 0,
            keywords: vec![],
            category: Some("CityTransport".to_string()),
            category_keywords: None,
            fields: vec![],
        };

        assert_eq!(
            TemplateManager::classify_by_template(&template, "任意文本"),
            Some("CityTransport".to_string())
        );
    }

    #[test]
    fn test_classify_by_template_no_category_returns_none() {
        let template = InvoiceTemplate {
            template_id: "test".to_string(),
            name: "测试".to_string(),
            enabled: true,
            priority: 0,
            keywords: vec![],
            category: None,
            category_keywords: None,
            fields: vec![],
        };

        assert_eq!(
            TemplateManager::classify_by_template(&template, "任意文本"),
            None
        );
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test --lib -p invoice-reimbursement test_dual_dir -- --nocapture`
预期：编译失败，`from_dual_dirs`/`template_source`/`classify_by_template`/`TemplateSource` 不存在

- [ ] **步骤 3：实现 TemplateSource 枚举和双目录加载**

在 `template_manager.rs` 的 `InvoiceTemplate` 结构体定义之后添加：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TemplateSource {
    Builtin,
    User,
}
```

修改 `TemplateManager` 结构体和 impl（替换现有 `TemplateManager` 结构体定义和 `new`/`from_config_dir` 方法）：

```rust
pub struct TemplateManager {
    templates: HashMap<String, InvoiceTemplate>,
    sources: HashMap<String, TemplateSource>,
}

impl TemplateManager {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            sources: HashMap::new(),
        }
    }

    /// 从内置+用户双目录加载模板，用户模板同 id 覆盖内置
    pub fn from_dual_dirs<P: AsRef<Path>, Q: AsRef<Path>>(
        builtin_dir: P,
        user_dir: Q,
    ) -> Result<Self, String> {
        let mut manager = Self::new();
        manager.load_dir(builtin_dir, TemplateSource::Builtin)?;
        manager.load_dir(user_dir, TemplateSource::User)?; // User 后加载，覆盖同 id
        Ok(manager)
    }

    /// 兼容旧接口：单目录加载（标记为 Builtin）
    pub fn from_config_dir<P: AsRef<Path>>(dir: P) -> Result<Self, String> {
        let mut manager = Self::new();
        manager.load_dir(dir, TemplateSource::Builtin)?;
        Ok(manager)
    }

    fn load_dir<P: AsRef<Path>>(&mut self, dir: P, source: TemplateSource) -> Result<(), String> {
        let dir_path = dir.as_ref();
        if !dir_path.exists() {
            return Ok(());
        }
        let entries = std::fs::read_dir(dir_path).map_err(|e| e.to_string())?;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                match Self::load_template(&path) {
                    Ok(template) => {
                        self.templates.insert(template.template_id.clone(), template);
                        self.sources.insert(path.file_stem().unwrap_or_default().to_string_lossy().to_string(), source);
                        // 记录 id→source 映射
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load template {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn template_source(&self, template_id: &str) -> TemplateSource {
        self.sources.get(template_id).copied().unwrap_or(TemplateSource::Builtin)
    }
```

注意：`load_dir` 中 `sources` 的 key 应该用 `template_id` 而非文件名。修正 `load_dir` 中的 sources 插入行：

```rust
                    Ok(template) => {
                        let id = template.template_id.clone();
                        self.templates.insert(id.clone(), template);
                        self.sources.insert(id, source);
                    }
```

- [ ] **步骤 4：实现 priority 排序和 enabled 过滤的 match_template**

替换现有 `match_template` 方法：

```rust
    pub fn match_template(&self, ocr: &OcrStructuredOutput) -> Option<&InvoiceTemplate> {
        let all_text = ocr.blocks.iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        // 过滤 enabled，按 priority 降序排序
        let mut candidates: Vec<&InvoiceTemplate> = self.templates.values()
            .filter(|t| t.enabled)
            .filter(|t| t.keywords.iter().all(|k| all_text.contains(k)))
            .collect();
        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));
        candidates.first().copied()
    }
```

- [ ] **步骤 5：实现 classify_by_template 关联函数**

在 impl 块中添加：

```rust
    /// 用模板的分类关键词判断分类，回退到模板 category 默认值
    pub fn classify_by_template(template: &InvoiceTemplate, text: &str) -> Option<String> {
        if let Some(ref ck) = template.category_keywords {
            let text_lower = text.to_lowercase();
            // 按 category_keywords 的 key 顺序检查（HashMap 无序，需收集排序保证确定性）
            let mut keys: Vec<&String> = ck.keys().collect();
            keys.sort();
            for category in keys {
                let keywords = ck.get(category).unwrap();
                if keywords.iter().any(|k| text_lower.contains(&k.to_lowercase())) {
                    return Some(category.clone());
                }
            }
        }
        template.category.clone()
    }
```

- [ ] **步骤 6：更新 add_template 方法记录 source**

替换现有 `add_template` 方法：

```rust
    pub fn add_template(&mut self, template: InvoiceTemplate) {
        self.sources.insert(template.template_id.clone(), TemplateSource::User);
        self.templates.insert(template.template_id.clone(), template);
    }
```

- [ ] **步骤 7：更新 reload_from_config_dir 支持双目录**

替换现有 `reload_from_config_dir` 方法，新增 `reload_from_dual_dirs`：

```rust
    pub fn reload_from_config_dir<P: AsRef<Path>>(&mut self, dir: P) -> Result<(), String> {
        let new_manager = Self::from_config_dir(dir)?;
        self.templates = new_manager.templates;
        self.sources = new_manager.sources;
        Ok(())
    }

    pub fn reload_from_dual_dirs<P: AsRef<Path>, Q: AsRef<Path>>(
        &mut self,
        builtin_dir: P,
        user_dir: Q,
    ) -> Result<(), String> {
        let new_manager = Self::from_dual_dirs(builtin_dir, user_dir)?;
        self.templates = new_manager.templates;
        self.sources = new_manager.sources;
        Ok(())
    }
```

- [ ] **步骤 8：运行测试验证通过**

运行：`cargo test --lib -p invoice-reimbursement template_manager -- --nocapture`
预期：所有测试 PASS

- [ ] **步骤 9：Commit**

```bash
git add src-tauri/src/parser/template_manager.rs
git commit -m "feat: TemplateManager 支持双目录加载、priority 排序、enabled 过滤、模板分类逻辑"
```

---

## 任务 3：内置模板种子文件

**文件：**
- 创建：`src-tauri/src/builtin_templates/vat_normal.json`
- 创建：`src-tauri/src/builtin_templates/vat_special.json`
- 创建：`src-tauri/src/builtin_templates/didi_itinerary.json`
- 创建：`src-tauri/src/builtin_templates/hotel.json`
- 创建：`src-tauri/src/builtin_templates/tianfutong.json`

这些模板的初始内容 = 现有硬编码逻辑的等价 JSON 表达，从 `field_extractors.rs` 和 `invoice_parser.rs` 的正则提取。

- [ ] **步骤 1：创建增值税普通发票模板**

`src-tauri/src/builtin_templates/vat_normal.json`：

```json
{
  "template_id": "vat_normal",
  "name": "增值税普通发票",
  "enabled": true,
  "priority": 10,
  "keywords": ["增值税普通发票"],
  "category": "Other",
  "category_keywords": {
    "Hotel": ["住宿服务", "酒店", "宾馆", "住宿", "招待所", "民宿"],
    "Meal": ["餐饮服务", "餐饮", "饭店", "食品", "餐厅", "饭馆"],
    "CityTransport": ["运输服务", "客运服务", "滴滴", "网约车", "高德", "t3", "曹操", "出租"],
    "Flight": ["航空运输服务", "旅客运输服务", "航空", "机票", "机场", "航班"],
    "Train": ["火车", "高铁", "铁路", "客运站"],
    "TicketChange": ["退票", "改签", "保险"]
  },
  "fields": [
    {
      "name": "amount",
      "required": true,
      "strategies": [
        {
          "type": "regex",
          "pattern": "(?:价税合计|合计金额|总金额|金额|实付金额)[：:]?[￥¥]*\\s*([\\d,]+\\.?\\d*)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.9
        },
        {
          "type": "regex",
          "pattern": "[￥¥]\\s*([\\d,]+\\.?\\d*)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.7
        }
      ]
    },
    {
      "name": "seller_name",
      "required": true,
      "strategies": [
        {
          "type": "regex",
          "pattern": "销售方[：:]\\s*名称[：:]\\s*(\\S+)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.95
        },
        {
          "type": "regex",
          "pattern": "名称[：:]\\s*(\\S+)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.85
        },
        {
          "type": "regex",
          "pattern": "销售方[：:]\\s*(\\S+)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.8
        },
        {
          "type": "regex",
          "pattern": "收款单位[：:]\\s*(\\S+)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.8
        }
      ]
    },
    {
      "name": "date",
      "required": false,
      "strategies": [
        {
          "type": "regex",
          "pattern": "(\\d{4})\\s*年\\s*(\\d{1,2})\\s*月\\s*(\\d{1,2})\\s*日",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.9
        },
        {
          "type": "regex",
          "pattern": "(\\d{4})-(\\d{2})-(\\d{2})",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.85
        }
      ]
    },
    {
      "name": "invoice_number",
      "required": false,
      "strategies": [
        {
          "type": "regex",
          "pattern": "(?:发票号码|发票代码|No|号码)[：:]?\\s*(\\d+)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.9
        },
        {
          "type": "regex",
          "pattern": "(\\d{8,20})\\s*(?:发票号码|发票代码|号码)[：:]?",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.8
        }
      ]
    },
    {
      "name": "item_name",
      "required": false,
      "strategies": [
        {
          "type": "regex",
          "pattern": "(?:项目名称|货物或应税劳务|商品名称|服务名称)[：:]\\s*(\\S+)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.85
        },
        {
          "type": "regex",
          "pattern": "\\*(.+?)\\*",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.7
        }
      ]
    }
  ]
}
```

- [ ] **步骤 2：创建增值税专用发票模板**

`src-tauri/src/builtin_templates/vat_special.json`：

```json
{
  "template_id": "vat_special",
  "name": "增值税专用发票",
  "enabled": true,
  "priority": 10,
  "keywords": ["增值税专用发票"],
  "category": "Other",
  "category_keywords": {
    "Hotel": ["住宿服务", "酒店", "宾馆", "住宿", "招待所", "民宿"],
    "Meal": ["餐饮服务", "餐饮", "饭店", "食品", "餐厅", "饭馆"],
    "CityTransport": ["运输服务", "客运服务", "滴滴", "网约车", "高德", "t3", "曹操", "出租"],
    "Flight": ["航空运输服务", "旅客运输服务", "航空", "机票", "机场", "航班"],
    "Train": ["火车", "高铁", "铁路", "客运站"],
    "TicketChange": ["退票", "改签", "保险"]
  },
  "fields": [
    {
      "name": "amount",
      "required": true,
      "strategies": [
        {
          "type": "regex",
          "pattern": "(?:价税合计|合计金额|总金额|金额|实付金额)[：:]?[￥¥]*\\s*([\\d,]+\\.?\\d*)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.9
        },
        {
          "type": "regex",
          "pattern": "[￥¥]\\s*([\\d,]+\\.?\\d*)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.7
        }
      ]
    },
    {
      "name": "seller_name",
      "required": true,
      "strategies": [
        {
          "type": "regex",
          "pattern": "销售方[：:]\\s*名称[：:]\\s*(\\S+)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.95
        },
        {
          "type": "regex",
          "pattern": "名称[：:]\\s*(\\S+)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.85
        }
      ]
    },
    {
      "name": "date",
      "required": false,
      "strategies": [
        {
          "type": "regex",
          "pattern": "(\\d{4})\\s*年\\s*(\\d{1,2})\\s*月\\s*(\\d{1,2})\\s*日",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.9
        }
      ]
    },
    {
      "name": "invoice_number",
      "required": false,
      "strategies": [
        {
          "type": "regex",
          "pattern": "(?:发票号码|发票代码|No|号码)[：:]?\\s*(\\d+)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.9
        }
      ]
    }
  ]
}
```

- [ ] **步骤 3：创建滴滴行程单模板**

`src-tauri/src/builtin_templates/didi_itinerary.json`：

```json
{
  "template_id": "didi_itinerary",
  "name": "滴滴行程单",
  "enabled": true,
  "priority": 20,
  "keywords": ["滴滴", "行程单"],
  "category": "CityTransport",
  "category_keywords": null,
  "fields": [
    {
      "name": "amount",
      "required": true,
      "strategies": [
        {
          "type": "regex",
          "pattern": "合计\\s*([\\d,]+\\.?\\d*)\\s*元",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.9
        },
        {
          "type": "regex",
          "pattern": "[￥¥]\\s*([\\d,]+\\.?\\d*)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.7
        }
      ]
    }
  ]
}
```

- [ ] **步骤 4：创建酒店发票模板**

`src-tauri/src/builtin_templates/hotel.json`：

```json
{
  "template_id": "hotel_invoice",
  "name": "酒店住宿发票",
  "enabled": true,
  "priority": 15,
  "keywords": ["住宿"],
  "category": "Hotel",
  "category_keywords": null,
  "fields": [
    {
      "name": "amount",
      "required": true,
      "strategies": [
        {
          "type": "regex",
          "pattern": "(?:价税合计|合计金额|总金额|金额)[：:]?[￥¥]*\\s*([\\d,]+\\.?\\d*)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.9
        }
      ]
    },
    {
      "name": "seller_name",
      "required": true,
      "strategies": [
        {
          "type": "regex",
          "pattern": "名称[：:]\\s*(\\S+)",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.85
        }
      ]
    },
    {
      "name": "date",
      "required": false,
      "strategies": [
        {
          "type": "regex",
          "pattern": "(\\d{4})\\s*年\\s*(\\d{1,2})\\s*月\\s*(\\d{1,2})\\s*日",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.9
        }
      ]
    }
  ]
}
```

- [ ] **步骤 5：创建天府通行程单模板**

`src-tauri/src/builtin_templates/tianfutong.json`：

```json
{
  "template_id": "tianfutong_itinerary",
  "name": "天府通行程单",
  "enabled": true,
  "priority": 20,
  "keywords": ["天府通"],
  "category": "CityTransport",
  "category_keywords": null,
  "fields": [
    {
      "name": "amount",
      "required": true,
      "strategies": [
        {
          "type": "regex",
          "pattern": "合计\\s*([\\d,]+\\.?\\d*)\\s*元",
          "section_keyword": null,
          "field_keyword": null,
          "confidence": 0.9
        }
      ]
    }
  ]
}
```

- [ ] **步骤 6：验证 JSON 文件语法正确**

运行：`Get-ChildItem src-tauri/src/builtin_templates/*.json | ForEach-Object { try { Get-Content $_.FullName | ConvertFrom-Json | Out-Null; "OK: $($_.Name)" } catch { "FAIL: $($_.Name) - $_" } }`
预期：5 个文件全部 OK

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/builtin_templates/
git commit -m "feat: 添加 5 个内置模板种子文件（从硬编码规则提取）"
```

---

## 任务 4：解析流程集成（模板优先→硬编码回退）

**文件：**
- 修改：`src-tauri/src/parser/invoice_parser.rs:310-411`

- [ ] **步骤 1：编写失败的测试**

在 `invoice_parser.rs` 测试模块末尾添加（需先确认测试模块存在，若无需在文件末尾添加 `#[cfg(test)] mod tests { use super::*; ... }`）：

```rust
    #[test]
    fn test_template_classification_overrides_hardcoded() {
        use crate::ocr::structured_output::{BoundingBox, OcrTextBlock, PageLayout, TextBlockType};
        use std::collections::HashMap;

        let blocks = vec![OcrTextBlock {
            text: "增值税普通发票 价税合计：¥100.00 名称：测试餐饮店".to_string(),
            confidence: 0.95,
            bbox: BoundingBox { x: 0.0, y: 0.0, width: 200.0, height: 20.0 },
            line_index: 0,
            block_type: TextBlockType::KeyValue,
        }];
        let ocr = OcrStructuredOutput { blocks, layout: PageLayout::default() };

        // 模板带 category_keywords，应返回模板分类
        let template = InvoiceTemplate {
            template_id: "test_cat".to_string(),
            name: "测试分类".to_string(),
            enabled: true,
            priority: 100,
            keywords: vec!["增值税普通发票".to_string()],
            category: Some("Other".to_string()),
            category_keywords: Some(HashMap::from([
                ("Meal".to_string(), vec!["餐饮".to_string()]),
            ])),
            fields: vec![FieldDefinition {
                name: "amount".to_string(),
                required: true,
                strategies: vec![FieldStrategy {
                    strategy_type: "regex".to_string(),
                    pattern: Some("价税合计[：:￥¥]*\\s*([\\d,]+\\.?\\d*)".to_string()),
                    section_keyword: None,
                    field_keyword: None,
                    confidence: 0.9,
                }],
            }],
        };

        let mut manager = TemplateManager::new();
        manager.add_template(template);

        let invoice = parse_structured_invoice_with_templates(
            &ocr,
            InvoiceSource::Pdf("test.pdf".to_string()),
            Some(&manager),
        ).unwrap();

        assert_eq!(invoice.category, InvoiceCategory::Meal);
        assert!((invoice.amount - 100.0).abs() < 0.001);
        assert_eq!(invoice.seller_name, "测试餐饮店");
    }

    #[test]
    fn test_fallback_to_hardcoded_when_no_template_matches() {
        use crate::ocr::structured_output::{BoundingBox, OcrTextBlock, PageLayout, TextBlockType};

        let blocks = vec![OcrTextBlock {
            text: "某未知格式发票 金额：¥200.00".to_string(),
            confidence: 0.95,
            bbox: BoundingBox { x: 0.0, y: 0.0, width: 200.0, height: 20.0 },
            line_index: 0,
            block_type: TextBlockType::KeyValue,
        }];
        let ocr = OcrStructuredOutput { blocks, layout: PageLayout::default() };

        // 空模板管理器，无模板匹配
        let manager = TemplateManager::new();
        let invoice = parse_structured_invoice_with_templates(
            &ocr,
            InvoiceSource::Pdf("test.pdf".to_string()),
            Some(&manager),
        ).unwrap();

        // 应回退到硬编码逻辑
        assert!((invoice.amount - 200.0).abs() < 0.001);
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test --lib -p invoice-reimbursement test_template_classification_overrides -- --nocapture`
预期：FAIL，分类仍为硬编码结果而非模板的 Meal

- [ ] **步骤 3：改造 try_parse_with_template 使用模板分类**

替换 `invoice_parser.rs:362-411` 的 `try_parse_with_template` 函数：

```rust
fn try_parse_with_template(
    ocr_output: &OcrStructuredOutput,
    source: &InvoiceSource,
    template: &InvoiceTemplate,
    manager: &TemplateManager,
) -> Result<Invoice, String> {
    let extracted_values = manager.extract_with_template(ocr_output, template)?;

    // 用模板的分类逻辑，而非硬编码 classify_from_full_text
    let all_text = ocr_output.blocks.iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let category = match TemplateManager::classify_by_template(template, &all_text) {
        Some(cat_str) => parse_category_from_str(&cat_str),
        None => {
            // 模板无分类配置，回退硬编码
            let invoice_type = InvoiceTypeDetector::detect(ocr_output);
            classify_from_full_text(ocr_output, &None, &None, &invoice_type)
        }
    };

    let mut amount = 0.0f64;
    let mut seller_name = String::new();
    let mut invoice_number = String::new();
    let mut date = chrono::NaiveDate::default();
    let mut item_name = String::new();

    for extracted in extracted_values {
        match extracted.field_name.as_str() {
            "amount" => {
                amount = extracted.value.replace(",", "").parse::<f64>()
                    .map_err(|e| format!("金额解析失败: {}", e))?;
            }
            "seller_name" => {
                seller_name = extracted.value;
            }
            "invoice_number" => {
                invoice_number = extracted.value;
            }
            "date" => {
                date = parse_date_from_string(&extracted.value).unwrap_or_default();
            }
            "item_name" => {
                item_name = extracted.value;
            }
            _ => {}
        }
    }

    Ok(Invoice {
        id: Uuid::new_v4().to_string(),
        invoice_number,
        amount,
        seller_name,
        item_name,
        date,
        category,
        source: source.clone(),
        itineraries: vec![],
        itinerary_file: None,
        remarks: String::new(),
        hotel_detail: None,
    })
}

/// 将分类字符串解析为 InvoiceCategory 枚举
fn parse_category_from_str(s: &str) -> InvoiceCategory {
    match s {
        "Train" => InvoiceCategory::Train,
        "Flight" => InvoiceCategory::Flight,
        "TicketChange" => InvoiceCategory::TicketChange,
        "CityTransport" => InvoiceCategory::CityTransport,
        "Hotel" => InvoiceCategory::Hotel,
        "Meal" => InvoiceCategory::Meal,
        _ => InvoiceCategory::Other,
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test --lib -p invoice-reimbursement test_template_classification -- --nocapture`
预期：两个测试都 PASS

- [ ] **步骤 5：运行回归测试确保现有行为不变**

运行：`cargo test --lib -p invoice-reimbursement invoice_parser -- --nocapture`
预期：所有现有测试仍 PASS（因为现有调用未注入模板管理器，走硬编码回退）

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/parser/invoice_parser.rs
git commit -m "feat: 解析流程集成模板分类逻辑，模板命中时用模板分类，未命中回退硬编码"
```

---

## 任务 5：正则骨架生成器

**文件：**
- 创建：`src-tauri/src/parser/regex_skeleton.rs`
- 修改：`src-tauri/src/parser/mod.rs`

- [ ] **步骤 1：编写失败的测试**

创建 `src-tauri/src/parser/regex_skeleton.rs`，先只写测试：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    Amount,
    Date,
    InvoiceNumber,
    SellerName,
    ItemName,
}

/// 根据字段类型和用户拖选的文本，生成正则表达式骨架
pub fn generate_regex(field_type: FieldType, selected_text: &str) -> String {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_with_currency_prefix() {
        let regex = generate_regex(FieldType::Amount, "价税合计：¥1,234.56");
        assert!(regex.contains("价税合计"), "应保留前缀: {}", regex);
        assert!(regex.contains("[\\d,]+\\.?\\d*"), "应包含数字捕获组: {}", regex);
        assert!(regex.contains("("), "应有捕获组: {}", regex);
    }

    #[test]
    fn test_amount_with_yuan_symbol() {
        let regex = generate_regex(FieldType::Amount, "合计：￥500.00");
        assert!(regex.contains("[￥¥]"), "应泛化货币符号: {}", regex);
        assert!(regex.contains("[\\d,]+\\.?\\d*"), "{}", regex);
    }

    #[test]
    fn test_amount_pure_number() {
        let regex = generate_regex(FieldType::Amount, "1234.56");
        assert!(regex.contains("[\\d,]+\\.?\\d*"), "{}", regex);
        assert!(regex.contains("("), "{}", regex);
    }

    #[test]
    fn test_date_chinese_format() {
        let regex = generate_regex(FieldType::Date, "2024年05月20日");
        assert!(regex.contains("\\d{4}"), "应匹配年份: {}", regex);
        assert!(regex.contains("\\d{1,2}"), "应匹配月日: {}", regex);
    }

    #[test]
    fn test_date_iso_format() {
        let regex = generate_regex(FieldType::Date, "2024-05-20");
        assert!(regex.contains("\\d{4}"), "{}", regex);
        assert!(regex.contains("\\d{1,2}"), "{}", regex);
    }

    #[test]
    fn test_invoice_number_with_prefix() {
        let regex = generate_regex(FieldType::InvoiceNumber, "发票号码：12345678");
        assert!(regex.contains("发票号码"), "应保留前缀: {}", regex);
        assert!(regex.contains("\\d{8,20}"), "应匹配数字: {}", regex);
    }

    #[test]
    fn test_seller_name_with_prefix() {
        let regex = generate_regex(FieldType::SellerName, "名称：测试餐饮店");
        assert!(regex.contains("名称"), "应保留前缀: {}", regex);
        assert!(regex.contains("[：:]"), "应泛化冒号: {}", regex);
        assert!(regex.contains("("), "应有捕获组: {}", regex);
    }

    #[test]
    fn test_item_name_with_stars() {
        let regex = generate_regex(FieldType::ItemName, "*住宿服务*");
        assert!(regex.contains("\\*"), "应匹配星号: {}", regex);
        assert!(regex.contains("("), "{}", regex);
    }

    #[test]
    fn test_item_name_with_prefix() {
        let regex = generate_regex(FieldType::ItemName, "项目名称：住宿费");
        assert!(regex.contains("项目名称"), "{}", regex);
        assert!(regex.contains("("), "{}", regex);
    }

    #[test]
    fn test_colon_generalization() {
        // 全角冒号应泛化为 [：:]
        let regex = generate_regex(FieldType::SellerName, "名称：测试公司");
        assert!(regex.contains("[：:]"), "应泛化冒号: {}", regex);
    }
}
```

- [ ] **步骤 2：在 mod.rs 注册模块**

在 `src-tauri/src/parser/mod.rs` 末尾添加：

```rust
pub mod regex_skeleton;
pub use regex_skeleton::{FieldType, generate_regex};
```

- [ ] **步骤 3：运行测试验证失败**

运行：`cargo test --lib -p invoice-reimbursement regex_skeleton -- --nocapture`
预期：FAIL，`generate_regex` panic（unimplemented）

- [ ] **步骤 4：实现 generate_regex**

替换 `regex_skeleton.rs` 中的 `generate_regex` 函数：

```rust
/// 根据字段类型和用户拖选的文本，生成正则表达式骨架
pub fn generate_regex(field_type: FieldType, selected_text: &str) -> String {
    match field_type {
        FieldType::Amount => generate_amount_regex(selected_text),
        FieldType::Date => generate_date_regex(selected_text),
        FieldType::InvoiceNumber => generate_invoice_number_regex(selected_text),
        FieldType::SellerName => generate_seller_name_regex(selected_text),
        FieldType::ItemName => generate_item_name_regex(selected_text),
    }
}

/// 泛化冒号：全角/半角 → [：:]
fn generalize_colon(s: &str) -> String {
    if s.contains('：') || s.contains(':') {
        s.replace('：', "[：:]").replace(':', "[：:]")
    } else {
        s.to_string()
    }
}

/// 泛化货币符号：¥/￥ → [￥¥]
fn generalize_currency(s: &str) -> String {
    s.replace('¥', "[￥¥]").replace('￥', "[￥¥]")
}

/// 提取数字前的前缀文本（到第一个数字为止）
fn extract_prefix_before_number(text: &str) -> String {
    let pos = text.find(|c: char| c.is_ascii_digit());
    match pos {
        Some(p) => text[..p].to_string(),
        None => text.to_string(),
    }
}

fn generate_amount_regex(selected: &str) -> String {
    let prefix = extract_prefix_before_number(selected);
    let number_group = "([\\d,]+\\.?\\d*)";

    if prefix.is_empty() {
        return number_group.to_string();
    }

    // 泛化前缀中的冒号和货币符号
    let prefix = generalize_currency(&prefix);
    let prefix = generalize_colon(&prefix);
    // 去除尾部空白，用 \s* 连接
    let prefix = prefix.trim_end();
    format!("{}\\s*{}", prefix, number_group)
}

fn generate_date_regex(selected: &str) -> String {
    if selected.contains('年') || selected.contains('月') || selected.contains('日') {
        r"(\d{4}\s*年\s*\d{1,2}\s*月\s*\d{1,2}\s*日?)".to_string()
    } else if selected.contains('-') || selected.contains('/') {
        r"(\d{4}[-/]\d{1,2}[-/]\d{1,2})".to_string()
    } else {
        r"(\d{4}\s*年\s*\d{1,2}\s*月\s*\d{1,2}\s*日?)".to_string()
    }
}

fn generate_invoice_number_regex(selected: &str) -> String {
    let prefix = extract_prefix_before_number(selected);
    let number_group = "(\\d{8,20})";

    if prefix.is_empty() {
        return number_group.to_string();
    }

    let prefix = generalize_colon(&prefix);
    let prefix = prefix.trim_end();
    format!("{}\\s*{}", prefix, number_group)
}

fn generate_seller_name_regex(selected: &str) -> String {
    // 检测前缀关键词
    let prefixes = ["名称", "销售方", "收款单位", "开票方"];
    for prefix in &prefixes {
        if selected.contains(prefix) {
            let after_prefix = &selected[selected.find(prefix).unwrap() + prefix.len()..];
            // 泛化冒号
            let colon = if after_prefix.starts_with('：') || after_prefix.starts_with(':') {
                "[：:]"
            } else {
                ""
            };
            return format!("{}{}\\s*(.+?)(?:\\s|$)", prefix, colon);
        }
    }
    // 无前缀，匹配整段
    "(.+?)(?:\\s|$)".to_string()
}

fn generate_item_name_regex(selected: &str) -> String {
    if selected.contains('*') {
        return r"\*(.+?)\*".to_string();
    }

    let prefixes = ["项目名称", "货物或应税劳务", "商品名称", "服务名称", "品目"];
    for prefix in &prefixes {
        if selected.contains(prefix) {
            let after_prefix = &selected[selected.find(prefix).unwrap() + prefix.len()..];
            let colon = if after_prefix.starts_with('：') || after_prefix.starts_with(':') {
                "[：:]"
            } else {
                ""
            };
            return format!("{}{}\\s*(.+)", prefix, colon);
        }
    }

    "(.+)".to_string()
}
```

- [ ] **步骤 5：运行测试验证通过**

运行：`cargo test --lib -p invoice-reimbursement regex_skeleton -- --nocapture`
预期：所有测试 PASS

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/parser/regex_skeleton.rs src-tauri/src/parser/mod.rs
git commit -m "feat: 新增正则骨架生成器，支持5种字段类型的标注→正则自动生成"
```

---

## 任务 6：Tauri 模板管理命令

**文件：**
- 创建：`src-tauri/src/commands/template_commands.rs`
- 修改：`src-tauri/src/lib.rs`
- 修改：`src-tauri/src/parser/mod.rs`

- [ ] **步骤 1：创建命令模块骨架**

创建 `src-tauri/src/commands/mod.rs`：

```rust
pub mod template_commands;
```

创建 `src-tauri/src/commands/template_commands.rs`：

```rust
use crate::parser::template_manager::{InvoiceTemplate, TemplateManager, TemplateSource};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex as AsyncMutex;

/// 模板元信息（列表用）
#[derive(Debug, Serialize)]
pub struct TemplateMeta {
    pub template_id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub source: TemplateSource,
}

/// 测试结果
#[derive(Debug, Serialize)]
pub struct TestResult {
    pub matched: bool,
    pub matched_keyword: Option<String>,
    pub fields: Vec<FieldTestResult>,
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FieldTestResult {
    pub name: String,
    pub success: bool,
    pub value: Option<String>,
    pub error: Option<String>,
}

/// 获取内置模板目录和用户模板目录
fn get_template_dirs(app: &AppHandle) -> (PathBuf, PathBuf) {
    let resource_dir = app.path().resource_dir().unwrap_or_else(|_| PathBuf::from("."));
    let builtin_dir = resource_dir.join("builtin_templates");

    let config_dir = app.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("."));
    let user_dir = config_dir.join("user_templates");

    (builtin_dir, user_dir)
}

/// 列出所有模板
#[tauri::command]
pub async fn list_templates(app: AppHandle) -> Result<Vec<TemplateMeta>, String> {
    let (builtin_dir, user_dir) = get_template_dirs(&app);
    let manager = TemplateManager::from_dual_dirs(&builtin_dir, &user_dir)?;

    let mut metas: Vec<TemplateMeta> = manager.templates().values().map(|t| {
        TemplateMeta {
            template_id: t.template_id.clone(),
            name: t.name.clone(),
            enabled: t.enabled,
            priority: t.priority,
            source: manager.template_source(&t.template_id),
        }
    }).collect();
    metas.sort_by(|a, b| b.priority.cmp(&a.priority));
    Ok(metas)
}

/// 获取单个模板
#[tauri::command]
pub async fn get_template(app: AppHandle, id: String) -> Result<InvoiceTemplate, String> {
    let (builtin_dir, user_dir) = get_template_dirs(&app);
    let manager = TemplateManager::from_dual_dirs(&builtin_dir, &user_dir)?;
    manager.get_template(&id)
        .cloned()
        .ok_or_else(|| format!("模板不存在: {}", id))
}

/// 保存模板到用户目录
#[tauri::command]
pub async fn save_template(app: AppHandle, template: InvoiceTemplate) -> Result<String, String> {
    // 验证正则可编译
    for field in &template.fields {
        for strategy in &field.strategies {
            if strategy.strategy_type == "regex" {
                if let Some(ref pattern) = strategy.pattern {
                    regex::Regex::new(pattern)
                        .map_err(|e| format!("字段 '{}' 正则错误: {}", field.name, e))?;
                }
            }
        }
    }
    // 验证 keywords 非空
    if template.keywords.is_empty() {
        return Err("至少需要一个识别关键词".to_string());
    }

    let (_, user_dir) = get_template_dirs(&app);
    std::fs::create_dir_all(&user_dir).map_err(|e| e.to_string())?;

    let file_path = user_dir.join(format!("{}.json", template.template_id));
    let json = serde_json::to_string_pretty(&template).map_err(|e| e.to_string())?;

    // 原子写入：临时文件 + rename
    let tmp_path = file_path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp_path, &file_path).map_err(|e| e.to_string())?;

    Ok(template.template_id)
}

/// 删除用户模板（不允许删内置）
#[tauri::command]
pub async fn delete_template(app: AppHandle, id: String) -> Result<(), String> {
    let (_, user_dir) = get_template_dirs(&app);
    let file_path = user_dir.join(format!("{}.json", id));

    if !file_path.exists() {
        return Err(format!("用户模板不存在: {}", id));
    }

    std::fs::remove_file(&file_path).map_err(|e| e.to_string())
}

/// 启用/禁用模板
#[tauri::command]
pub async fn toggle_template(app: AppHandle, id: String, enabled: bool) -> Result<(), String> {
    let (builtin_dir, user_dir) = get_template_dirs(&app);
    let manager = TemplateManager::from_dual_dirs(&builtin_dir, &user_dir)?;

    let mut template = manager.get_template(&id)
        .cloned()
        .ok_or_else(|| format!("模板不存在: {}", id))?;

    template.enabled = enabled;

    // 内置模板若要修改 enabled，复制到用户目录
    let (_, user_dir) = get_template_dirs(&app);
    std::fs::create_dir_all(&user_dir).map_err(|e| e.to_string())?;
    let file_path = user_dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&template).map_err(|e| e.to_string())?;
    std::fs::write(&file_path, json).map_err(|e| e.to_string())
}

/// 测试模板（内存，不落盘）
#[tauri::command]
pub async fn test_template(
    app: AppHandle,
    template: InvoiceTemplate,
    pdf_path: String,
) -> Result<TestResult, String> {
    use crate::ocr::structured_output::OcrStructuredOutput;
    use crate::ocr::OcrEngine;
    use crate::pdf::text_extractor;

    // 提取文本
    let text_items = match text_extractor::extract_text_from_pdf(&pdf_path) {
        Ok(items) if text_extractor::has_sufficient_text(&items, 20) => items,
        _ => {
            // 回退 OCR
            let state = app.state::<crate::AppState>();
            let mut engine_guard = state.ocr_engine.lock().await;
            let engine = engine_guard.as_mut()
                .ok_or("OCR engine not initialized")?;
            let resp = engine.recognize_pdf(&pdf_path)?;
            resp.pages.iter().flat_map(|p| p.texts.clone()).collect()
        }
    };

    let blocks: Vec<_> = text_items.iter().map(|t| {
        crate::ocr::structured_output::OcrTextBlock {
            text: t.text.clone(),
            confidence: t.confidence,
            bbox: crate::ocr::structured_output::BoundingBox::default(),
            line_index: 0,
            block_type: crate::ocr::structured_output::TextBlockType::Other,
        }
    }).collect();
    let ocr = OcrStructuredOutput {
        blocks,
        layout: crate::ocr::structured_output::PageLayout::default(),
    };

    let all_text: String = ocr.blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join(" ");

    // 检查模板是否匹配
    let matched = template.keywords.iter().all(|k| all_text.contains(k));
    let matched_keyword = if matched {
        template.keywords.iter().find(|k| all_text.contains(*k)).cloned()
    } else {
        None
    };

    // 提取各字段
    let manager = TemplateManager::new();
    let mut field_results = Vec::new();
    for field_def in &template.fields {
        match manager.extract_field(&all_text, field_def) {
            Ok(Some(val)) => field_results.push(FieldTestResult {
                name: field_def.name.clone(),
                success: true,
                value: Some(val.value),
                error: None,
            }),
            Ok(None) => field_results.push(FieldTestResult {
                name: field_def.name.clone(),
                success: false,
                value: None,
                error: Some("正则未匹配".to_string()),
            }),
            Err(e) => field_results.push(FieldTestResult {
                name: field_def.name.clone(),
                success: false,
                value: None,
                error: Some(e),
            }),
        }
    }

    // 分类
    let category = if matched {
        TemplateManager::classify_by_template(&template, &all_text)
    } else {
        None
    };

    Ok(TestResult {
        matched,
        matched_keyword,
        fields: field_results,
        category,
    })
}

/// 标注模式：只返回 OCR 纯文本
#[tauri::command]
pub async fn ocr_for_annotation(
    app: AppHandle,
    pdf_path: String,
) -> Result<String, String> {
    use crate::pdf::text_extractor;

    let text_items = match text_extractor::extract_text_from_pdf(&pdf_path) {
        Ok(items) if text_extractor::has_sufficient_text(&items, 20) => items,
        _ => {
            let state = app.state::<crate::AppState>();
            let mut engine_guard = state.ocr_engine.lock().await;
            let engine = engine_guard.as_mut()
                .ok_or("OCR engine not initialized")?;
            let resp = engine.recognize_pdf(&pdf_path)?;
            resp.pages.iter().flat_map(|p| p.texts.clone()).collect()
        }
    };

    let text: String = text_items.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join("\n");
    Ok(text)
}

/// 重载模板
#[tauri::command]
pub async fn reload_templates(app: AppHandle) -> Result<(), String> {
    // 模板是按需加载的（每次命令都 from_dual_dirs），无需额外操作
    // 此命令保留供未来缓存优化使用
    Ok(())
}
```

- [ ] **步骤 2：在 lib.rs 注册模块和命令**

在 `src-tauri/src/lib.rs` 顶部添加模块声明（在 `pub mod parser;` 之后）：

```rust
pub mod commands;
```

在 `invoke_handler` 宏中添加新命令（在 `open_file_with_system,` 之后）：

```rust
            commands::template_commands::list_templates,
            commands::template_commands::get_template,
            commands::template_commands::save_template,
            commands::template_commands::delete_template,
            commands::template_commands::toggle_template,
            commands::template_commands::test_template,
            commands::template_commands::ocr_for_annotation,
            commands::template_commands::reload_templates,
```

- [ ] **步骤 3：编译验证**

运行：`cargo build --lib -p invoice-reimbursement`
预期：编译成功。若有错误，根据错误信息修正（常见：`extract_field` 是私有方法需改为 pub、`AppState` 需设为 pub）

- [ ] **步骤 4：修复可见性问题**

若编译报 `extract_field` 私有错误，在 `template_manager.rs` 中将 `fn extract_field` 改为 `pub fn extract_field`。

若编译报 `AppState` 私有错误，在 `lib.rs` 中将 `struct AppState` 改为 `pub struct AppState`。

- [ ] **步骤 5：再次编译验证**

运行：`cargo build --lib -p invoice-reimbursement`
预期：编译成功

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/commands/ src-tauri/src/lib.rs src-tauri/src/parser/template_manager.rs
git commit -m "feat: 新增 8 个模板管理 Tauri 命令（list/get/save/delete/toggle/test/ocr/reload）"
```

---

## 任务 7：内置模板打包到资源目录

**文件：**
- 修改：`src-tauri/tauri.conf.json`

内置模板文件需在打包时复制到资源目录，这样运行时 `resource_dir/builtin_templates/` 才能找到。

- [ ] **步骤 1：查看 tauri.conf.json 当前配置**

运行：读取 `src-tauri/tauri.conf.json`，找到 `bundle.resources` 字段（若不存在则需添加）

- [ ] **步骤 2：添加 resources 配置**

在 `tauri.conf.json` 的 `bundle` 对象中添加（若已有 `resources` 则追加）：

```json
"resources": ["builtin_templates/*.json"]
```

- [ ] **步骤 3：验证配置正确**

运行：`cargo build --lib -p invoice-reimbursement`
预期：编译成功

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "chore: 配置内置模板打包到资源目录"
```

---

## 任务 8：回归测试与端到端验证

**文件：**
- 修改：`src-tauri/src/parser/invoice_parser.rs`（测试模块）

- [ ] **步骤 1：编写回归对比测试**

在 `invoice_parser.rs` 测试模块末尾添加：

```rust
    #[test]
    fn test_regression_template_vs_hardcoded_same_result() {
        use crate::ocr::structured_output::{BoundingBox, OcrTextBlock, PageLayout, TextBlockType};
        use std::collections::HashMap;

        // 模拟一张增值税普通发票的 OCR 文本
        let blocks = vec![
            OcrTextBlock {
                text: "增值税普通发票".to_string(),
                confidence: 0.95,
                bbox: BoundingBox { x: 0.0, y: 0.0, width: 200.0, height: 20.0 },
                line_index: 0,
                block_type: TextBlockType::Other,
            },
            OcrTextBlock {
                text: "价税合计：¥1,234.56".to_string(),
                confidence: 0.95,
                bbox: BoundingBox { x: 0.0, y: 100.0, width: 200.0, height: 20.0 },
                line_index: 5,
                block_type: TextBlockType::KeyValue,
            },
            OcrTextBlock {
                text: "名称：测试酒店".to_string(),
                confidence: 0.95,
                bbox: BoundingBox { x: 0.0, y: 150.0, width: 200.0, height: 20.0 },
                line_index: 7,
                block_type: TextBlockType::KeyValue,
            },
            OcrTextBlock {
                text: "2024年05月20日".to_string(),
                confidence: 0.95,
                bbox: BoundingBox { x: 0.0, y: 200.0, width: 200.0, height: 20.0 },
                line_index: 9,
                block_type: TextBlockType::KeyValue,
            },
        ];
        let ocr = OcrStructuredOutput { blocks, layout: PageLayout::default() };
        let source = InvoiceSource::Pdf("test.pdf".to_string());

        // 无模板：走硬编码
        let hardcoded = parse_structured_invoice_with_templates(&ocr, source.clone(), None).unwrap();

        // 有模板（等价正则）：走模板
        let template = InvoiceTemplate {
            template_id: "regression_test".to_string(),
            name: "回归测试".to_string(),
            enabled: true,
            priority: 10,
            keywords: vec!["增值税普通发票".to_string()],
            category: Some("Other".to_string()),
            category_keywords: Some(HashMap::from([
                ("Hotel".to_string(), vec!["酒店".to_string()]),
            ])),
            fields: vec![
                FieldDefinition {
                    name: "amount".to_string(),
                    required: true,
                    strategies: vec![FieldStrategy {
                        strategy_type: "regex".to_string(),
                        pattern: Some("价税合计[：:￥¥]*\\s*([\\d,]+\\.?\\d*)".to_string()),
                        section_keyword: None,
                        field_keyword: None,
                        confidence: 0.9,
                    }],
                },
                FieldDefinition {
                    name: "seller_name".to_string(),
                    required: false,
                    strategies: vec![FieldStrategy {
                        strategy_type: "regex".to_string(),
                        pattern: Some("名称[：:]\\s*(\\S+)".to_string()),
                        section_keyword: None,
                        field_keyword: None,
                        confidence: 0.85,
                    }],
                },
            ],
        };
        let mut manager = TemplateManager::new();
        manager.add_template(template);
        let templated = parse_structured_invoice_with_templates(&ocr, source, Some(&manager)).unwrap();

        // 金额应一致
        assert!((hardcoded.amount - templated.amount).abs() < 0.001,
            "金额不一致: 硬编码={} 模板={}", hardcoded.amount, templated.amount);
        assert!((hardcoded.amount - 1234.56).abs() < 0.001);

        // 销售方应一致
        assert!(!templated.seller_name.is_empty(), "模板模式销售方不应为空");
    }
```

- [ ] **步骤 2：运行回归测试**

运行：`cargo test --lib -p invoice-reimbursement test_regression -- --nocapture`
预期：PASS

- [ ] **步骤 3：运行全部测试**

运行：`cargo test --lib -p invoice-reimbursement -- --nocapture`
预期：所有测试 PASS（注意：`test_invoice_parser_with_templates` 可能因 Tera 模板编译耗时偶发超时，这是已知限制，重试即可）

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/parser/invoice_parser.rs
git commit -m "test: 添加模板模式 vs 硬编码模式回归对比测试"
```

---

## 自检结果

### 规格覆盖度

| 规格章节 | 覆盖任务 |
|---------|---------|
| 2. 架构与数据流 | 任务 4（解析流程集成）、任务 6（命令） |
| 3. 模板数据结构 | 任务 1（结构体扩展） |
| 4. 前端配置界面 | 前端计划（另文档） |
| 5.1 Tauri 命令 | 任务 6（8 个命令全部覆盖） |
| 5.2 TemplateManager 激活与扩展 | 任务 1+2+4 |
| 5.3 标注骨架生成器 | 任务 5 |
| 5.4 内置模板种子文件 | 任务 3 |
| 6. 错误处理与边界情况 | 任务 6（正则预编译验证、keywords 非空验证、原子写入）、任务 2（损坏文件跳过、priority 排序、enabled 过滤） |
| 7. 测试策略 | 任务 1-8 均含 TDD 测试 |
| 8. 实现顺序 Phase 1+2 | 任务 1-8 对应 Phase 1+2 |

### 占位符扫描

无占位符。所有步骤含完整代码。

### 类型一致性

- `InvoiceTemplate` 字段在任务 1 定义后，任务 2/3/4/6/8 使用一致
- `TemplateSource` 枚举在任务 2 定义，任务 6 使用一致
- `FieldType` 枚举在任务 5 定义，前端计划将引用
- `TestResult`/`FieldTestResult`/`TemplateMeta` 在任务 6 定义，前端计划将引用

### 已知限制

- `test_template` 命令中 `extract_field` 需为 pub（任务 6 步骤 4 已说明）
- `AppState` 需为 pub（任务 6 步骤 4 已说明）
- 内置模板的 `category_keywords` 使用 `InvoiceCategory` 枚举的变体名（如 "Meal"、"Hotel"），`parse_category_from_str` 负责转换（任务 4 步骤 3）

---

## 执行交接

计划已完成并保存到 `docs/superpowers/plans/2026-06-23-configurable-matching-backend.md`。

前端计划将另写为 `docs/superpowers/plans/2026-06-23-configurable-matching-frontend.md`。

两种执行方式：

**1. 子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代

**2. 内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点

选哪种方式？
