use crate::parser::template_manager::{InvoiceTemplate, TemplateManager, TemplateSource};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

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
pub async fn reload_templates(_app: AppHandle) -> Result<(), String> {
    // 模板是按需加载的（每次命令都 from_dual_dirs），无需额外操作
    // 此命令保留供未来缓存优化使用
    Ok(())
}

/// 标注模式：根据字段类型和拖选文本生成正则骨架
#[tauri::command]
pub async fn generate_regex_skeleton(
    field_type: String,
    selected_text: String,
) -> Result<String, String> {
    use crate::parser::regex_skeleton::{FieldType, generate_regex};

    let ft = match field_type.as_str() {
        "Amount" => FieldType::Amount,
        "Date" => FieldType::Date,
        "InvoiceNumber" => FieldType::InvoiceNumber,
        "SellerName" => FieldType::SellerName,
        "ItemName" => FieldType::ItemName,
        _ => return Err(format!("未知字段类型: {}", field_type)),
    };

    Ok(generate_regex(ft, &selected_text))
}
