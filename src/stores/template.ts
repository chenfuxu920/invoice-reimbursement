import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { TemplateMeta, InvoiceTemplate, TestResult } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const useTemplateStore = defineStore('template', () => {
  const templates = ref<TemplateMeta[]>([])
  const loading = ref(false)
  const currentTemplate = ref<InvoiceTemplate | null>(null)

  /// 加载模板列表
  async function loadTemplates() {
    loading.value = true
    try {
      templates.value = await invoke<TemplateMeta[]>('list_templates')
    } catch (e) {
      console.error('加载模板列表失败:', e)
      throw e
    } finally {
      loading.value = false
    }
  }

  /// 获取单个模板详情
  async function loadTemplate(id: string) {
    try {
      currentTemplate.value = await invoke<InvoiceTemplate>('get_template', { id })
    } catch (e) {
      console.error('加载模板失败:', e)
      throw e
    }
  }

  /// 保存模板
  async function saveTemplate(template: InvoiceTemplate) {
    try {
      await invoke('save_template', { template })
      await loadTemplates()
    } catch (e) {
      console.error('保存模板失败:', e)
      throw e
    }
  }

  /// 删除模板
  async function deleteTemplate(id: string) {
    try {
      await invoke('delete_template', { id })
      await loadTemplates()
    } catch (e) {
      console.error('删除模板失败:', e)
      throw e
    }
  }

  /// 启用/禁用模板
  async function toggleTemplate(id: string, enabled: boolean) {
    try {
      await invoke('toggle_template', { id, enabled })
      const t = templates.value.find(t => t.template_id === id)
      if (t) t.enabled = enabled
    } catch (e) {
      console.error('切换模板状态失败:', e)
      throw e
    }
  }

  /// 测试模板
  async function testTemplate(template: InvoiceTemplate, pdfPath: string) {
    return await invoke<TestResult>('test_template', { template, pdfPath })
  }

  /// 标注模式：获取 OCR 文本
  async function getOcrText(pdfPath: string) {
    return await invoke<string>('ocr_for_annotation', { pdfPath })
  }

  /// 标注模式：生成正则骨架
  async function generateRegex(fieldType: string, selectedText: string) {
    return await invoke<string>('generate_regex_skeleton', { fieldType, selectedText })
  }

  /// 创建空白模板
  function createBlankTemplate(): InvoiceTemplate {
    return {
      template_id: 'user_' + Date.now(),
      name: '新模板',
      enabled: true,
      priority: 5,
      keywords: [],
      category: 'Other',
      category_keywords: {},
      fields: [
        { name: 'amount', required: true, strategies: [{ type: 'regex', pattern: '', section_keyword: null, field_keyword: null, confidence: 0.9 }] },
        { name: 'seller_name', required: true, strategies: [{ type: 'regex', pattern: '', section_keyword: null, field_keyword: null, confidence: 0.85 }] },
        { name: 'date', required: false, strategies: [{ type: 'regex', pattern: '', section_keyword: null, field_keyword: null, confidence: 0.8 }] },
        { name: 'invoice_number', required: false, strategies: [{ type: 'regex', pattern: '', section_keyword: null, field_keyword: null, confidence: 0.9 }] },
        { name: 'item_name', required: false, strategies: [{ type: 'regex', pattern: '', section_keyword: null, field_keyword: null, confidence: 0.7 }] },
      ],
    }
  }

  return {
    templates,
    loading,
    currentTemplate,
    loadTemplates,
    loadTemplate,
    saveTemplate,
    deleteTemplate,
    toggleTemplate,
    testTemplate,
    getOcrText,
    generateRegex,
    createBlankTemplate,
  }
})
