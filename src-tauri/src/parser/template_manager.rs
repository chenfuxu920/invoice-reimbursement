use crate::ocr::structured_output::OcrStructuredOutput;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TemplateSource {
    Builtin,
    User,
}

#[derive(Debug, Clone)]
pub struct ExtractedValue {
    pub field_name: String,
    pub value: String,
    pub confidence: f64,
    pub strategy: String,
}

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
                        let id = template.template_id.clone();
                        self.templates.insert(id.clone(), template);
                        self.sources.insert(id, source);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load template {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(())
    }

    fn load_template<P: AsRef<Path>>(path: P) -> Result<InvoiceTemplate, String> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("Failed to read file: {}", e))?;
        let template: InvoiceTemplate = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;
        Ok(template)
    }

    pub fn templates(&self) -> &HashMap<String, InvoiceTemplate> {
        &self.templates
    }

    pub fn get_template(&self, template_id: &str) -> Option<&InvoiceTemplate> {
        self.templates.get(template_id)
    }

    pub fn template_source(&self, template_id: &str) -> TemplateSource {
        self.sources.get(template_id).copied().unwrap_or(TemplateSource::Builtin)
    }

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

    pub fn extract_with_template(
        &self,
        ocr: &OcrStructuredOutput,
        template: &InvoiceTemplate,
    ) -> Result<Vec<ExtractedValue>, String> {
        let all_text = ocr.blocks.iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let mut results = Vec::new();

        for field_def in &template.fields {
            let extracted = self.extract_field(&all_text, field_def)?;
            
            if field_def.required && extracted.is_none() {
                return Err(format!("Required field '{}' not found", field_def.name));
            }
            
            if let Some(value) = extracted {
                results.push(value);
            }
        }

        Ok(results)
    }

    fn extract_field(&self, text: &str, field_def: &FieldDefinition) -> Result<Option<ExtractedValue>, String> {
        for strategy in &field_def.strategies {
            let result = match strategy.strategy_type.as_str() {
                "regex" => self.apply_regex_strategy(text, strategy)?,
                "section_field" => self.apply_section_field_strategy(text, strategy)?,
                _ => None,
            };

            if let Some(value) = result {
                return Ok(Some(ExtractedValue {
                    field_name: field_def.name.clone(),
                    value,
                    confidence: strategy.confidence,
                    strategy: strategy.strategy_type.clone(),
                }));
            }
        }

        Ok(None)
    }

    fn apply_regex_strategy(&self, text: &str, strategy: &FieldStrategy) -> Result<Option<String>, String> {
        let pattern = strategy.pattern.as_ref()
            .ok_or_else(|| "Regex strategy missing pattern".to_string())?;
        
        let re = Regex::new(pattern)
            .map_err(|e| format!("Invalid regex pattern: {}", e))?;

        if let Some(caps) = re.captures(text) {
            if let Some(m) = caps.get(1) {
                return Ok(Some(m.as_str().trim().to_string()));
            }
        }

        Ok(None)
    }

    fn apply_section_field_strategy(&self, text: &str, strategy: &FieldStrategy) -> Result<Option<String>, String> {
        let section_keyword = strategy.section_keyword.as_ref()
            .ok_or_else(|| "Section field strategy missing section_keyword".to_string())?;
        let field_keyword = strategy.field_keyword.as_ref()
            .ok_or_else(|| "Section field strategy missing field_keyword".to_string())?;

        let lines: Vec<&str> = text.lines().collect();
        let mut in_section = false;
        let mut section_lines: Vec<&str> = Vec::new();

        for line in &lines {
            if line.contains(section_keyword) {
                in_section = true;
                section_lines.push(*line);
                continue;
            }

            if in_section {
                if line.contains("价税合计") || line.contains("合计金额") || line.contains("合计")
                    || line.contains("购买方信息") || line.contains("销售方信息") {
                    break;
                }
                section_lines.push(*line);
            }
        }

        if !section_lines.is_empty() {
            let section_text = section_lines.join("\n");
            
            let field_pattern = format!(r"{}[：:]+\s*(\S+)", regex::escape(field_keyword));
            let field_re = Regex::new(&field_pattern)
                .map_err(|e| format!("Invalid field regex: {}", e))?;

            if let Some(caps) = field_re.captures(&section_text) {
                if let Some(m) = caps.get(1) {
                    return Ok(Some(m.as_str().trim().to_string()));
                }
            }
            
            let field_pattern2 = format!(r"{}[：:]+\s*(.+)", regex::escape(field_keyword));
            let field_re2 = Regex::new(&field_pattern2)
                .map_err(|e| format!("Invalid field regex: {}", e))?;

            if let Some(caps) = field_re2.captures(&section_text) {
                if let Some(m) = caps.get(1) {
                    let value = m.as_str().trim();
                    let value = value.split_whitespace().next().unwrap_or(value);
                    return Ok(Some(value.to_string()));
                }
            }
        }

        Ok(None)
    }

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

    pub fn add_template(&mut self, template: InvoiceTemplate) {
        self.sources.insert(template.template_id.clone(), TemplateSource::User);
        self.templates.insert(template.template_id.clone(), template);
    }

    pub fn remove_template(&mut self, template_id: &str) -> Option<InvoiceTemplate> {
        self.templates.remove(template_id)
    }
}

impl Default for TemplateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::structured_output::{BoundingBox, OcrTextBlock, PageLayout, TextBlockType};
    use std::fs;
    use tempfile::TempDir;

    fn create_ocr_output(texts: Vec<&str>) -> OcrStructuredOutput {
        let blocks = texts
            .iter()
            .enumerate()
            .map(|(i, text)| OcrTextBlock {
                text: text.to_string(),
                confidence: 0.95,
                bbox: BoundingBox {
                    x: 0.0,
                    y: (i * 20) as f64,
                    width: 200.0,
                    height: 20.0,
                },
                line_index: i,
                block_type: TextBlockType::KeyValue,
            })
            .collect();

        OcrStructuredOutput {
            blocks,
            layout: PageLayout::default(),
        }
    }

    #[test]
    fn test_template_manager_empty() {
        let manager = TemplateManager::new();
        assert!(manager.templates().is_empty());
    }

    #[test]
    fn test_load_template_from_dir() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("test.json");
        
        let template_json = r#"{
            "template_id": "test_template",
            "name": "测试模板",
            "keywords": ["测试"],
            "fields": [
                {
                    "name": "amount",
                    "required": true,
                    "strategies": [
                        {
                            "type": "regex",
                            "pattern": "金额[：:]+\\s*([\\d,]+\\.?\\d*)",
                            "confidence": 0.9
                        }
                    ]
                }
            ]
        }"#;
        
        fs::write(&template_path, template_json).unwrap();
        
        let manager = TemplateManager::from_config_dir(temp_dir.path()).unwrap();
        assert_eq!(manager.templates().len(), 1);
        assert!(manager.get_template("test_template").is_some());
    }

    #[test]
    fn test_match_template() {
        let mut manager = TemplateManager::new();
        
        let template = InvoiceTemplate {
            template_id: "vat_invoice".to_string(),
            name: "增值税发票".to_string(),
            enabled: true,
            priority: 0,
            keywords: vec!["增值税".to_string(), "发票".to_string()],
            category: None,
            category_keywords: None,
            fields: vec![],
        };
        
        manager.add_template(template);
        
        let ocr = create_ocr_output(vec!["增值税电子发票", "金额：100.00"]);
        let matched = manager.match_template(&ocr);
        
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().template_id, "vat_invoice");
    }

    #[test]
    fn test_no_match_template() {
        let mut manager = TemplateManager::new();
        
        let template = InvoiceTemplate {
            template_id: "flight".to_string(),
            name: "机票".to_string(),
            enabled: true,
            priority: 0,
            keywords: vec!["航空".to_string(), "机票".to_string()],
            category: None,
            category_keywords: None,
            fields: vec![],
        };
        
        manager.add_template(template);
        
        let ocr = create_ocr_output(vec!["酒店住宿费", "金额：200.00"]);
        let matched = manager.match_template(&ocr);
        
        assert!(matched.is_none());
    }

    #[test]
    fn test_regex_extraction() {
        let manager = TemplateManager::new();
        
        let field_def = FieldDefinition {
            name: "amount".to_string(),
            required: true,
            strategies: vec![FieldStrategy {
                strategy_type: "regex".to_string(),
                pattern: Some("价税合计[：:￥¥]*\\s*([\\d,]+\\.?\\d*)".to_string()),
                section_keyword: None,
                field_keyword: None,
                confidence: 0.9,
            }],
        };

        let text = "价税合计：¥1045.24";
        let result = manager.extract_field(text, &field_def).unwrap();
        
        assert!(result.is_some());
        let extracted = result.unwrap();
        assert_eq!(extracted.field_name, "amount");
        assert_eq!(extracted.value, "1045.24");
    }

    #[test]
    fn test_section_field_extraction() {
        let manager = TemplateManager::new();
        
        let field_def = FieldDefinition {
            name: "seller_name".to_string(),
            required: true,
            strategies: vec![FieldStrategy {
                strategy_type: "section_field".to_string(),
                pattern: None,
                section_keyword: Some("销售方信息".to_string()),
                field_keyword: Some("名称".to_string()),
                confidence: 0.95,
            }],
        };

        let text = "销售方信息\n名称：四川景澜酒店管理有限公司\n统一社会信用代码：1234567890";
        let result = manager.extract_field(text, &field_def).unwrap();
        
        assert!(result.is_some());
        let extracted = result.unwrap();
        assert_eq!(extracted.field_name, "seller_name");
        assert_eq!(extracted.value, "四川景澜酒店管理有限公司");
    }

    #[test]
    fn test_extract_with_template() {
        let manager = TemplateManager::new();
        
        let template = InvoiceTemplate {
            template_id: "test".to_string(),
            name: "测试".to_string(),
            enabled: true,
            priority: 0,
            keywords: vec!["测试".to_string()],
            category: None,
            category_keywords: None,
            fields: vec![
                FieldDefinition {
                    name: "amount".to_string(),
                    required: true,
                    strategies: vec![FieldStrategy {
                        strategy_type: "regex".to_string(),
                        pattern: Some("金额[：:￥¥]*\\s*([\\d,]+\\.?\\d*)".to_string()),
                        section_keyword: None,
                        field_keyword: None,
                        confidence: 0.9,
                    }],
                },
            ],
        };

        let ocr = create_ocr_output(vec!["测试发票", "金额：500.00"]);
        let results = manager.extract_with_template(&ocr, &template).unwrap();
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].field_name, "amount");
        assert_eq!(results[0].value, "500.00");
    }

    #[test]
    fn test_required_field_missing() {
        let manager = TemplateManager::new();
        
        let template = InvoiceTemplate {
            template_id: "test".to_string(),
            name: "测试".to_string(),
            enabled: true,
            priority: 0,
            keywords: vec![],
            category: None,
            category_keywords: None,
            fields: vec![
                FieldDefinition {
                    name: "amount".to_string(),
                    required: true,
                    strategies: vec![FieldStrategy {
                        strategy_type: "regex".to_string(),
                        pattern: Some("金额[：:￥¥]*\\s*([\\d,]+\\.?\\d*)".to_string()),
                        section_keyword: None,
                        field_keyword: None,
                        confidence: 0.9,
                    }],
                },
            ],
        };

        let ocr = create_ocr_output(vec!["其他文本", "无金额信息"]);
        let result = manager.extract_with_template(&ocr, &template);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Required field"));
    }

    #[test]
    fn test_reload_templates() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("test.json");
        
        let template_json = r#"{
            "template_id": "reload_test",
            "name": "重载测试",
            "keywords": ["重载"],
            "fields": []
        }"#;
        
        fs::write(&template_path, template_json).unwrap();
        
        let mut manager = TemplateManager::new();
        assert!(manager.templates().is_empty());
        
        manager.reload_from_config_dir(temp_dir.path()).unwrap();
        assert_eq!(manager.templates().len(), 1);
        assert!(manager.get_template("reload_test").is_some());
    }

    #[test]
    fn test_graceful_degradation() {
        let manager = TemplateManager::from_config_dir("/nonexistent/path").unwrap();
        assert!(manager.templates().is_empty());
    }

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
}
