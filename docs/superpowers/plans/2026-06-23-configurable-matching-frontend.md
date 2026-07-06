# 可配置发票匹配规则 - 前端实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 新增 `/templates` 路由，包含模板列表页、模板编辑器（手写+标注双模式）、实时测试面板，让用户可视化管理和配置发票匹配规则。

**架构：** Vue 3 + Pinia + Vue Router + Tailwind CSS。新增 `templates` store 管理模板状态，新增 `TemplateListView`/`TemplateEditorView` 两个视图，编辑器内含手写模式和标注模式切换，标注模式通过 `ocr_for_annotation` 命令获取 OCR 文本后在前端做拖选→调用后端正则骨架生成（后端已实现 `generate_regex`，但未暴露为 Tauri 命令，需补充）。

**技术栈：** Vue 3 (Composition API), Pinia, Vue Router, Tailwind CSS 4, @tauri-apps/api

**规格文档：** `docs/superpowers/specs/2026-06-23-configurable-invoice-matching-design.md`
**后端计划：** `docs/superpowers/plans/2026-06-23-configurable-matching-backend.md`

---

## 文件结构

**创建：**
- `src/types/template.ts` — 模板相关 TypeScript 类型定义
- `src/stores/template.ts` — 模板管理 Pinia store
- `src/views/TemplateListView.vue` — 模板列表页
- `src/views/TemplateEditorView.vue` — 模板编辑器（手写+标注双模式）
- `src/components/TemplateCard.vue` — 模板列表中的单个模板卡片
- `src/components/AnnotationPanel.vue` — 标注模式面板（OCR 文本拖选→生成正则）
- `src/components/TestPanel.vue` — 实时测试面板

**修改：**
- `src/router/index.ts` — 新增 `/templates` 和 `/templates/edit/:id?` 路由
- `src/App.vue` — 导航栏新增"模板"链接
- `src-tauri/src/commands/template_commands.rs` — 补充 `generate_regex_skeleton` 命令（后端计划遗漏）
- `src-tauri/src/lib.rs` — 注册新命令

---

## 任务 1：TypeScript 类型定义

**文件：**
- 创建：`src/types/template.ts`
- 修改：`src/types/index.ts`

- [ ] **步骤 1：创建模板类型文件**

创建 `src/types/template.ts`：

```typescript
/// 模板来源
export type TemplateSource = 'Builtin' | 'User'

/// 字段提取策略类型
export interface FieldStrategy {
  type: 'regex' | 'section_field'
  pattern: string | null
  section_keyword: string | null
  field_keyword: string | null
  confidence: number
}

/// 字段定义
export interface FieldDefinition {
  name: string
  required: boolean
  strategies: FieldStrategy[]
}

/// 发票模板
export interface InvoiceTemplate {
  template_id: string
  name: string
  enabled: boolean
  priority: number
  keywords: string[]
  category: string | null
  category_keywords: Record<string, string[]> | null
  fields: FieldDefinition[]
}

/// 模板元信息（列表用）
export interface TemplateMeta {
  template_id: string
  name: string
  enabled: boolean
  priority: number
  source: TemplateSource
}

/// 字段类型（标注模式用）
export type FieldType = 'Amount' | 'Date' | 'InvoiceNumber' | 'SellerName' | 'ItemName'

/// 单个字段测试结果
export interface FieldTestResult {
  name: string
  success: boolean
  value: string | null
  error: string | null
}

/// 模板测试结果
export interface TestResult {
  matched: boolean
  matched_keyword: string | null
  fields: FieldTestResult[]
  category: string | null
}

/// 标准字段名列表
export const STANDARD_FIELDS = ['amount', 'seller_name', 'date', 'invoice_number', 'item_name'] as const

/// 字段类型与字段名的映射（标注模式用）
export const FIELD_TYPE_MAP: Record<FieldType, string> = {
  Amount: 'amount',
  Date: 'date',
  InvoiceNumber: 'invoice_number',
  SellerName: 'seller_name',
  ItemName: 'item_name',
}

/// 字段类型中文标签
export const FIELD_TYPE_LABELS: Record<FieldType, string> = {
  Amount: '金额',
  Date: '日期',
  InvoiceNumber: '发票号',
  SellerName: '销售方',
  ItemName: '商品名',
}

/// 发票分类选项
export const CATEGORY_OPTIONS = [
  { value: 'Train', label: '高铁/车船票' },
  { value: 'Flight', label: '飞机票' },
  { value: 'TicketChange', label: '退改签/保险费' },
  { value: 'CityTransport', label: '市内交通' },
  { value: 'Hotel', label: '住宿费' },
  { value: 'Meal', label: '餐饮费' },
  { value: 'Other', label: '其他' },
]
```

- [ ] **步骤 2：在 types/index.ts 导出**

读取 `src/types/index.ts`，在末尾添加：

```typescript
export * from './template'
```

- [ ] **步骤 3：验证类型编译**

运行：`npx vue-tsc --noEmit`
预期：无错误

- [ ] **步骤 4：Commit**

```bash
git add src/types/template.ts src/types/index.ts
git commit -m "feat: 新增模板相关 TypeScript 类型定义"
```

---

## 任务 2：补充后端 generate_regex_skeleton 命令

后端计划遗漏了将 `generate_regex` 暴露为 Tauri 命令的步骤，标注模式需要它。

**文件：**
- 修改：`src-tauri/src/commands/template_commands.rs`
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：添加命令函数**

在 `src-tauri/src/commands/template_commands.rs` 末尾添加：

```rust
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
```

- [ ] **步骤 2：在 lib.rs 注册命令**

在 `invoke_handler` 宏中添加（在 `reload_templates,` 之后）：

```rust
            commands::template_commands::generate_regex_skeleton,
```

- [ ] **步骤 3：编译验证**

运行：`cargo build --lib -p invoice-reimbursement`
预期：编译成功

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/commands/template_commands.rs src-tauri/src/lib.rs
git commit -m "feat: 暴露 generate_regex_skeleton 为 Tauri 命令供标注模式调用"
```

---

## 任务 3：Pinia Store

**文件：**
- 创建：`src/stores/template.ts`

- [ ] **步骤 1：创建 store**

创建 `src/stores/template.ts`：

```typescript
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
```

- [ ] **步骤 2：验证类型编译**

运行：`npx vue-tsc --noEmit`
预期：无错误

- [ ] **步骤 3：Commit**

```bash
git add src/stores/template.ts
git commit -m "feat: 新增模板管理 Pinia store"
```

---

## 任务 4：路由与导航

**文件：**
- 修改：`src/router/index.ts`
- 修改：`src/App.vue`

- [ ] **步骤 1：添加路由**

将 `src/router/index.ts` 的 routes 数组替换为：

```typescript
const routes = [
  { path: '/', name: 'home', component: () => import('../views/HomeView.vue') },
  { path: '/import', name: 'import', component: () => import('../views/ImportView.vue') },
  { path: '/match', name: 'match', component: () => import('../views/MatchView.vue') },
  { path: '/export', name: 'export', component: () => import('../views/ExportView.vue') },
  { path: '/templates', name: 'templates', component: () => import('../views/TemplateListView.vue') },
  { path: '/templates/edit/:id?', name: 'template-edit', component: () => import('../views/TemplateEditorView.vue') },
]
```

- [ ] **步骤 2：添加导航链接**

在 `src/App.vue` 的 `<nav>` 中，在"导出"链接之后添加：

```html
      <router-link to="/templates" class="nav-link">模板</router-link>
```

- [ ] **步骤 3：创建占位视图（确保路由可访问）**

创建 `src/views/TemplateListView.vue`（占位，任务 5 实现）：

```vue
<template>
  <div class="max-w-4xl mx-auto">
    <h2 class="text-2xl font-bold mb-6">模板管理</h2>
    <p class="text-gray-500">（待实现）</p>
  </div>
</template>

<script setup lang="ts">
</script>
```

创建 `src/views/TemplateEditorView.vue`（占位，任务 6 实现）：

```vue
<template>
  <div class="max-w-4xl mx-auto">
    <h2 class="text-2xl font-bold mb-6">编辑模板</h2>
    <p class="text-gray-500">（待实现）</p>
  </div>
</template>

<script setup lang="ts">
</script>
```

- [ ] **步骤 4：验证编译**

运行：`npx vue-tsc --noEmit`
预期：无错误

- [ ] **步骤 5：Commit**

```bash
git add src/router/index.ts src/App.vue src/views/TemplateListView.vue src/views/TemplateEditorView.vue
git commit -m "feat: 新增 /templates 路由和导航链接"
```

---

## 任务 5：模板列表页

**文件：**
- 创建：`src/components/TemplateCard.vue`
- 修改：`src/views/TemplateListView.vue`

- [ ] **步骤 1：创建 TemplateCard 组件**

创建 `src/components/TemplateCard.vue`：

```vue
<template>
  <div class="border rounded-lg p-4 bg-white flex items-center justify-between hover:shadow-md transition-shadow">
    <div class="flex items-center gap-3">
      <span class="text-2xl">📄</span>
      <div>
        <div class="font-medium text-gray-800">{{ template.name }}</div>
        <div class="text-xs text-gray-400">
          ID: {{ template.template_id }} · 优先级: {{ template.priority }}
          <span v-if="template.source === 'Builtin'" class="ml-2 px-1.5 py-0.5 bg-blue-50 text-blue-600 rounded text-xs">内置</span>
          <span v-else class="ml-2 px-1.5 py-0.5 bg-green-50 text-green-600 rounded text-xs">用户</span>
        </div>
      </div>
    </div>
    <div class="flex items-center gap-2">
      <label class="flex items-center gap-1 text-sm text-gray-600 cursor-pointer">
        <input type="checkbox" :checked="template.enabled" @change="$emit('toggle', template.template_id, ($event.target as HTMLInputElement).checked)" class="rounded" />
        启用
      </label>
      <button @click="$emit('test', template.template_id)"
              class="px-3 py-1 rounded bg-gray-100 text-gray-700 hover:bg-gray-200 text-sm transition-colors">
        测试
      </button>
      <button v-if="template.source === 'Builtin'" @click="$emit('copy', template.template_id)"
              class="px-3 py-1 rounded bg-blue-50 text-blue-600 hover:bg-blue-100 text-sm transition-colors">
        复制
      </button>
      <button v-else @click="$emit('edit', template.template_id)"
              class="px-3 py-1 rounded bg-blue-50 text-blue-600 hover:bg-blue-100 text-sm transition-colors">
        编辑
      </button>
      <button v-if="template.source === 'User'" @click="$emit('delete', template.template_id)"
              class="px-3 py-1 rounded bg-red-50 text-red-600 hover:bg-red-100 text-sm transition-colors">
        删除
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { TemplateMeta } from '../types'

defineProps<{ template: TemplateMeta }>()
defineEmits<{
  toggle: [id: string, enabled: boolean]
  test: [id: string]
  copy: [id: string]
  edit: [id: string]
  delete: [id: string]
}>()
</script>
```

- [ ] **步骤 2：实现 TemplateListView**

替换 `src/views/TemplateListView.vue` 全部内容：

```vue
<template>
  <div class="max-w-4xl mx-auto">
    <div class="flex justify-between items-center mb-6">
      <h2 class="text-2xl font-bold">模板管理</h2>
      <button @click="handleCreate"
              class="px-4 py-2 rounded bg-blue-600 text-white hover:bg-blue-700 transition-colors text-sm font-medium">
        + 新建模板
      </button>
    </div>

    <div v-if="templateStore.loading" class="text-center py-8 text-gray-400">加载中...</div>

    <template v-else>
      <!-- 内置模板 -->
      <div v-if="builtinTemplates.length" class="mb-8">
        <h3 class="text-sm font-medium text-gray-500 mb-3">内置模板（只读）</h3>
        <div class="grid gap-3">
          <TemplateCard v-for="t in builtinTemplates" :key="t.template_id" :template="t"
            @toggle="handleToggle" @test="handleTest" @copy="handleCopy" @edit="handleEdit" @delete="handleDelete" />
        </div>
      </div>

      <!-- 用户模板 -->
      <div>
        <h3 class="text-sm font-medium text-gray-500 mb-3">我的模板</h3>
        <div v-if="userTemplates.length" class="grid gap-3">
          <TemplateCard v-for="t in userTemplates" :key="t.template_id" :template="t"
            @toggle="handleToggle" @test="handleTest" @copy="handleCopy" @edit="handleEdit" @delete="handleDelete" />
        </div>
        <div v-else class="text-center py-8 text-gray-400 border border-dashed rounded-lg">
          暂无自定义模板，点击右上角"新建模板"创建
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useTemplateStore } from '../stores/template'
import TemplateCard from '../components/TemplateCard.vue'

const router = useRouter()
const templateStore = useTemplateStore()

const builtinTemplates = computed(() => templateStore.templates.filter(t => t.source === 'Builtin'))
const userTemplates = computed(() => templateStore.templates.filter(t => t.source === 'User'))

onMounted(() => templateStore.loadTemplates())

async function handleToggle(id: string, enabled: boolean) {
  try {
    await templateStore.toggleTemplate(id, enabled)
  } catch (e) {
    alert(`切换失败: ${e}`)
  }
}

function handleTest(id: string) {
  router.push({ name: 'template-edit', params: { id }, query: { test: '1' } })
}

async function handleCopy(id: string) {
  // 加载内置模板，改 id 后跳转编辑器
  try {
    await templateStore.loadTemplate(id)
    if (templateStore.currentTemplate) {
      const copy = { ...templateStore.currentTemplate, template_id: 'user_' + Date.now(), name: templateStore.currentTemplate.name + ' (副本)' }
      await templateStore.saveTemplate(copy)
    }
  } catch (e) {
    alert(`复制失败: ${e}`)
  }
}

function handleEdit(id: string) {
  router.push({ name: 'template-edit', params: { id } })
}

async function handleDelete(id: string) {
  if (!confirm('确定删除此模板？')) return
  try {
    await templateStore.deleteTemplate(id)
  } catch (e) {
    alert(`删除失败: ${e}`)
  }
}

function handleCreate() {
  router.push({ name: 'template-edit' })
}
</script>
```

- [ ] **步骤 3：验证编译**

运行：`npx vue-tsc --noEmit`
预期：无错误

- [ ] **步骤 4：Commit**

```bash
git add src/components/TemplateCard.vue src/views/TemplateListView.vue
git commit -m "feat: 实现模板列表页（内置+用户分组、启用/禁用、复制/编辑/删除）"
```

---

## 任务 6：模板编辑器 - 手写模式

**文件：**
- 修改：`src/views/TemplateEditorView.vue`

- [ ] **步骤 1：实现编辑器主体（手写模式）**

替换 `src/views/TemplateEditorView.vue` 全部内容：

```vue
<template>
  <div class="max-w-4xl mx-auto">
    <!-- 顶部操作栏 -->
    <div class="flex justify-between items-center mb-6">
      <div class="flex items-center gap-3">
        <button @click="router.back()" class="text-gray-500 hover:text-gray-700">← 返回</button>
        <h2 class="text-2xl font-bold">{{ isNew ? '新建模板' : '编辑模板' }}</h2>
      </div>
      <div class="flex gap-2">
        <button @click="mode = mode === 'manual' ? 'annotation' : 'manual'"
                class="px-3 py-1.5 rounded bg-purple-50 text-purple-600 hover:bg-purple-100 text-sm font-medium transition-colors">
          {{ mode === 'manual' ? '切换到标注模式' : '切换到手写模式' }}
        </button>
        <button @click="handleSave" :disabled="saving"
                class="px-4 py-1.5 rounded bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50 text-sm font-medium transition-colors">
          {{ saving ? '保存中...' : '保存' }}
        </button>
      </div>
    </div>

    <!-- 手写模式 -->
    <div v-if="mode === 'manual'" class="space-y-6">
      <!-- 基本信息 -->
      <div class="bg-white border rounded-lg p-5 space-y-4">
        <h3 class="font-medium text-gray-700">基本信息</h3>
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label class="block text-sm text-gray-600 mb-1">模板名称</label>
            <input v-model="template.name" class="w-full border rounded px-3 py-2 text-sm" />
          </div>
          <div>
            <label class="block text-sm text-gray-600 mb-1">模板 ID</label>
            <input v-model="template.template_id" :disabled="!isNew" class="w-full border rounded px-3 py-2 text-sm disabled:bg-gray-50" />
          </div>
          <div>
            <label class="block text-sm text-gray-600 mb-1">优先级（数字越大越优先）</label>
            <input v-model.number="template.priority" type="number" class="w-full border rounded px-3 py-2 text-sm" />
          </div>
          <div>
            <label class="block text-sm text-gray-600 mb-1">默认分类</label>
            <select v-model="template.category" class="w-full border rounded px-3 py-2 text-sm">
              <option v-for="c in CATEGORY_OPTIONS" :key="c.value" :value="c.value">{{ c.label }}</option>
            </select>
          </div>
        </div>
        <div>
          <label class="block text-sm text-gray-600 mb-1">识别关键词（逗号分隔，全文包含任一即命中）</label>
          <input :value="template.keywords.join(', ')" @input="template.keywords = ($event.target as HTMLInputElement).value.split(',').map(s => s.trim()).filter(Boolean)"
                 class="w-full border rounded px-3 py-2 text-sm" placeholder="如: 增值税普通发票, 发票代码" />
        </div>
      </div>

      <!-- 分类关键词 -->
      <div class="bg-white border rounded-lg p-5 space-y-3">
        <h3 class="font-medium text-gray-700">分类关键词</h3>
        <p class="text-xs text-gray-400">发票文本命中某分类的关键词时，归为该分类。留空则使用上方默认分类。</p>
        <div v-for="(keywords, category) in template.category_keywords" :key="category" class="flex items-center gap-2">
          <select v-model="keywordsAsRecord[category]" @change="updateCategoryKey(category, keywordsAsRecord[category])"
                  class="border rounded px-2 py-1 text-sm w-32">
            <option v-for="c in CATEGORY_OPTIONS" :key="c.value" :value="c.value">{{ c.label }}</option>
          </select>
          <input :value="keywords.join(', ')" @input="updateCategoryKeywords(category, ($event.target as HTMLInputElement).value)"
                 class="flex-1 border rounded px-3 py-1.5 text-sm" placeholder="关键词，逗号分隔" />
          <button @click="removeCategory(category)" class="text-red-500 hover:text-red-700 text-sm">删除</button>
        </div>
        <button @click="addCategory" class="text-sm text-blue-600 hover:text-blue-800">+ 添加分类</button>
      </div>

      <!-- 字段提取规则 -->
      <div class="bg-white border rounded-lg p-5 space-y-3">
        <h3 class="font-medium text-gray-700">字段提取规则</h3>
        <div v-for="(field, idx) in template.fields" :key="idx" class="border-l-2 pl-3 space-y-2" :class="field.required ? 'border-blue-400' : 'border-gray-200'">
          <div class="flex items-center gap-3">
            <input v-model="field.name" class="border rounded px-2 py-1 text-sm w-40" placeholder="字段名" />
            <label class="flex items-center gap-1 text-sm text-gray-600">
              <input type="checkbox" v-model="field.required" /> 必填
            </label>
            <button @click="removeField(idx)" class="text-red-500 hover:text-red-700 text-sm ml-auto">删除字段</button>
          </div>
          <div v-for="(strategy, sIdx) in field.strategies" :key="sIdx" class="flex items-start gap-2">
            <input v-model="strategy.pattern" class="flex-1 border rounded px-3 py-1.5 text-sm font-mono" placeholder="正则表达式（含捕获组）" />
            <input v-model.number="strategy.confidence" type="number" step="0.05" min="0" max="1" class="border rounded px-2 py-1 text-sm w-20" title="置信度" />
            <button @click="removeStrategy(idx, sIdx)" class="text-red-500 hover:text-red-700 text-sm">✕</button>
          </div>
          <button @click="addStrategy(idx)" class="text-xs text-blue-600 hover:text-blue-800">+ 添加策略（按顺序回退）</button>
        </div>
        <button @click="addField" class="text-sm text-blue-600 hover:text-blue-800">+ 添加字段</button>
      </div>
    </div>

    <!-- 标注模式 -->
    <AnnotationPanel v-else :template="template" @update-field="onAnnotationUpdate" />

    <!-- 实时测试 -->
    <TestPanel :template="template" />
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useTemplateStore } from '../stores/template'
import { CATEGORY_OPTIONS, type InvoiceTemplate, type FieldDefinition, type FieldStrategy } from '../types'
import AnnotationPanel from '../components/AnnotationPanel.vue'
import TestPanel from '../components/TestPanel.vue'

const router = useRouter()
const route = useRoute()
const templateStore = useTemplateStore()

const id = computed(() => route.params.id as string | undefined)
const isNew = computed(() => !id.value)
const mode = ref<'manual' | 'annotation'>('manual')
const saving = ref(false)

const template = reactive<InvoiceTemplate>(templateStore.createBlankTemplate())

// category_keywords 的响应式代理（v-model 需要可写属性）
const keywordsAsRecord = computed(() => template.category_keywords || {})

onMounted(async () => {
  if (id.value) {
    try {
      await templateStore.loadTemplate(id.value)
      if (templateStore.currentTemplate) {
        Object.assign(template, JSON.parse(JSON.stringify(templateStore.currentTemplate)))
      }
    } catch (e) {
      alert(`加载模板失败: ${e}`)
      router.back()
    }
  }
  // 如果带 test=1 query，自动滚动到测试区
  if (route.query.test) {
    mode.value = 'manual'
  }
})

function updateCategoryKey(oldKey: string, newKey: string) {
  if (!template.category_keywords) return
  const vals = template.category_keywords[oldKey]
  delete template.category_keywords[oldKey]
  template.category_keywords[newKey] = vals
}

function updateCategoryKeywords(category: string, value: string) {
  if (!template.category_keywords) template.category_keywords = {}
  template.category_keywords[category] = value.split(',').map(s => s.trim()).filter(Boolean)
}

function addCategory() {
  if (!template.category_keywords) template.category_keywords = {}
  template.category_keywords['Other'] = []
}

function removeCategory(category: string) {
  if (template.category_keywords) delete template.category_keywords[category]
}

function addField() {
  template.fields.push({ name: 'new_field', required: false, strategies: [{ type: 'regex', pattern: '', section_keyword: null, field_keyword: null, confidence: 0.8 }] })
}

function removeField(idx: number) {
  template.fields.splice(idx, 1)
}

function addStrategy(fieldIdx: number) {
  template.fields[fieldIdx].strategies.push({ type: 'regex', pattern: '', section_keyword: null, field_keyword: null, confidence: 0.7 })
}

function removeStrategy(fieldIdx: number, strategyIdx: number) {
  template.fields[fieldIdx].strategies.splice(strategyIdx, 1)
}

function onAnnotationUpdate(fieldName: string, pattern: string) {
  const field = template.fields.find(f => f.name === fieldName)
  if (field && field.strategies.length > 0) {
    field.strategies[0].pattern = pattern
  }
}

async function handleSave() {
  if (template.keywords.length === 0) {
    alert('至少需要一个识别关键词')
    return
  }
  saving.value = true
  try {
    await templateStore.saveTemplate(template)
    router.push('/templates')
  } catch (e) {
    alert(`保存失败: ${e}`)
  } finally {
    saving.value = false
  }
}
</script>
```

- [ ] **步骤 2：创建 AnnotationPanel 和 TestPanel 占位组件**

创建 `src/components/AnnotationPanel.vue`（占位，任务 7 实现）：

```vue
<template>
  <div class="bg-white border rounded-lg p-5">
    <h3 class="font-medium text-gray-700 mb-3">标注模式</h3>
    <p class="text-gray-400 text-sm">（待实现）</p>
  </div>
</template>

<script setup lang="ts">
import type { InvoiceTemplate } from '../types'
defineProps<{ template: InvoiceTemplate }>()
defineEmits<{ 'update-field': [fieldName: string, pattern: string] }>()
</script>
```

创建 `src/components/TestPanel.vue`（占位，任务 8 实现）：

```vue
<template>
  <div class="bg-white border rounded-lg p-5 mt-6">
    <h3 class="font-medium text-gray-700 mb-3">实时测试</h3>
    <p class="text-gray-400 text-sm">（待实现）</p>
  </div>
</template>

<script setup lang="ts">
import type { InvoiceTemplate } from '../types'
defineProps<{ template: InvoiceTemplate }>()
</script>
```

- [ ] **步骤 3：验证编译**

运行：`npx vue-tsc --noEmit`
预期：无错误

- [ ] **步骤 4：Commit**

```bash
git add src/views/TemplateEditorView.vue src/components/AnnotationPanel.vue src/components/TestPanel.vue
git commit -m "feat: 实现模板编辑器手写模式（基本信息/分类关键词/字段正则编辑）"
```

---

## 任务 7：标注模式面板

**文件：**
- 修改：`src/components/AnnotationPanel.vue`

- [ ] **步骤 1：实现标注面板**

替换 `src/components/AnnotationPanel.vue` 全部内容：

```vue
<template>
  <div class="bg-white border rounded-lg p-5">
    <h3 class="font-medium text-gray-700 mb-4">标注模式</h3>

    <!-- 步骤1：上传 PDF -->
    <div class="mb-4">
      <label class="block text-sm text-gray-600 mb-1">① 上传发票 PDF</label>
      <div class="flex gap-2">
        <button @click="selectPdf" class="px-3 py-1.5 rounded bg-gray-100 text-gray-700 hover:bg-gray-200 text-sm transition-colors">
          选择文件
        </button>
        <span v-if="pdfPath" class="text-sm text-gray-500 self-center truncate flex-1">📄 {{ pdfPath }}</span>
      </div>
    </div>

    <!-- 步骤2：选择字段类型 -->
    <div class="mb-4">
      <label class="block text-sm text-gray-600 mb-1">② 选择要标注的字段类型</label>
      <div class="flex flex-wrap gap-2">
        <button v-for="ft in fieldTypes" :key="ft.value"
                @click="selectedFieldType = ft.value"
                :class="['px-3 py-1 rounded text-sm transition-colors', selectedFieldType === ft.value ? 'bg-blue-600 text-white' : 'bg-gray-100 text-gray-700 hover:bg-gray-200']">
          {{ ft.label }}
        </button>
      </div>
    </div>

    <!-- 步骤3：OCR 文本拖选 -->
    <div class="mb-4" v-if="ocrText">
      <label class="block text-sm text-gray-600 mb-1">③ 在下方文本中拖选该字段对应的内容</label>
      <div ref="ocrTextRef"
           @mouseup="handleTextSelection"
           class="border rounded p-3 bg-gray-50 text-sm font-mono whitespace-pre-wrap max-h-64 overflow-auto cursor-text select-text">
        {{ ocrText }}
      </div>
    </div>
    <div v-else-if="loadingOcr" class="text-center py-4 text-gray-400 text-sm">OCR 识别中...</div>

    <!-- 步骤4：生成的正则 -->
    <div v-if="generatedRegex !== null" class="mb-4">
      <label class="block text-sm text-gray-600 mb-1">④ 生成的正则（可手动修改）</label>
      <textarea v-model="editableRegex" rows="2"
                class="w-full border rounded px-3 py-2 text-sm font-mono"></textarea>
      <div class="flex gap-2 mt-2">
        <button @click="confirmField" class="px-3 py-1 rounded bg-green-600 text-white hover:bg-green-700 text-sm transition-colors">
          确认此字段
        </button>
        <button @click="generatedRegex = null" class="px-3 py-1 rounded bg-gray-100 text-gray-700 hover:bg-gray-200 text-sm transition-colors">
          取消
        </button>
      </div>
    </div>

    <!-- 标注进度 -->
    <div class="border-t pt-3">
      <div class="text-sm text-gray-600 mb-2">标注进度：</div>
      <div class="flex flex-wrap gap-2">
        <span v-for="ft in fieldTypes" :key="ft.value"
              :class="['px-2 py-0.5 rounded text-xs', isFieldAnnotated(ft.value) ? 'bg-green-50 text-green-600' : 'bg-gray-50 text-gray-400']">
          {{ ft.label }} {{ isFieldAnnotated(ft.value) ? '✓' : '✗' }}
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useTemplateStore } from '../stores/template'
import { FIELD_TYPE_LABELS, type InvoiceTemplate, type FieldType } from '../types'

const props = defineProps<{ template: InvoiceTemplate }>()
const emit = defineEmits<{ 'update-field': [fieldName: string, pattern: string] }>()

const templateStore = useTemplateStore()

const fieldTypes = computed(() =>
  (Object.keys(FIELD_TYPE_LABELS) as FieldType[]).map(v => ({ value: v, label: FIELD_TYPE_LABELS[v] }))
)

const pdfPath = ref('')
const ocrText = ref('')
const loadingOcr = ref(false)
const selectedFieldType = ref<FieldType>('Amount')
const generatedRegex = ref<string | null>(null)
const editableRegex = ref('')
const ocrTextRef = ref<HTMLElement | null>(null)

async function selectPdf() {
  const selected = await open({
    filters: [{ name: 'PDF', extensions: ['pdf'] }],
    multiple: false,
  })
  if (selected && typeof selected === 'string') {
    pdfPath.value = selected
    ocrText.value = ''
    generatedRegex.value = null
    loadingOcr.value = true
    try {
      ocrText.value = await templateStore.getOcrText(selected)
    } catch (e) {
      alert(`OCR 失败: ${e}`)
    } finally {
      loadingOcr.value = false
    }
  }
}

function handleTextSelection() {
  const selection = window.getSelection()
  if (!selection || selection.isCollapsed) return

  const selectedText = selection.toString().trim()
  if (!selectedText) return

  // 调用后端生成正则骨架
  templateStore.generateRegex(selectedFieldType.value, selectedText)
    .then(regex => {
      generatedRegex.value = regex
      editableRegex.value = regex
    })
    .catch(e => alert(`生成正则失败: ${e}`))
}

function confirmField() {
  if (!editableRegex.value) return
  // FieldType → 字段名映射
  const fieldNameMap: Record<FieldType, string> = {
    Amount: 'amount',
    Date: 'date',
    InvoiceNumber: 'invoice_number',
    SellerName: 'seller_name',
    ItemName: 'item_name',
  }
  emit('update-field', fieldNameMap[selectedFieldType.value], editableRegex.value)
  generatedRegex.value = null
}

function isFieldAnnotated(ft: FieldType): boolean {
  const fieldNameMap: Record<FieldType, string> = {
    Amount: 'amount',
    Date: 'date',
    InvoiceNumber: 'invoice_number',
    SellerName: 'seller_name',
    ItemName: 'item_name',
  }
  const field = props.template.fields.find(f => f.name === fieldNameMap[ft])
  return !!field && field.strategies.some(s => s.pattern && s.pattern.length > 0)
}
</script>
```

- [ ] **步骤 2：验证编译**

运行：`npx vue-tsc --noEmit`
预期：无错误

- [ ] **步骤 3：Commit**

```bash
git add src/components/AnnotationPanel.vue
git commit -m "feat: 实现标注模式面板（上传PDF→OCR→拖选文本→生成正则→可编辑→确认）"
```

---

## 任务 8：实时测试面板

**文件：**
- 修改：`src/components/TestPanel.vue`

- [ ] **步骤 1：实现测试面板**

替换 `src/components/TestPanel.vue` 全部内容：

```vue
<template>
  <div class="bg-white border rounded-lg p-5 mt-6">
    <h3 class="font-medium text-gray-700 mb-4">实时测试</h3>

    <!-- 选择测试文件 -->
    <div class="flex gap-2 mb-4">
      <button @click="selectTestPdf" class="px-3 py-1.5 rounded bg-gray-100 text-gray-700 hover:bg-gray-200 text-sm transition-colors">
        选择测试 PDF
      </button>
      <span v-if="testPdfPath" class="text-sm text-gray-500 self-center truncate flex-1">📄 {{ testPdfPath }}</span>
    </div>

    <!-- 测试按钮 -->
    <button v-if="testPdfPath" @click="runTest" :disabled="testing"
            class="px-4 py-1.5 rounded bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50 text-sm font-medium transition-colors mb-4">
      {{ testing ? '测试中...' : '开始测试' }}
    </button>

    <!-- 测试结果 -->
    <div v-if="result" class="space-y-3">
      <!-- 模板匹配 -->
      <div class="flex items-center gap-2">
        <span class="text-sm font-medium">模板匹配:</span>
        <span v-if="result.matched" class="text-green-600 text-sm">✅ 命中（关键词"{{ result.matched_keyword }}"匹配）</span>
        <span v-else class="text-red-600 text-sm">❌ 未命中（关键词都不匹配）</span>
      </div>

      <!-- 字段提取结果 -->
      <div>
        <div class="text-sm font-medium mb-2">字段提取结果:</div>
        <div class="space-y-1">
          <div v-for="field in result.fields" :key="field.name" class="flex items-center gap-2 text-sm">
            <span class="text-gray-600 w-28">{{ field.name }}:</span>
            <span v-if="field.success" class="text-green-600">✅ {{ field.value }}</span>
            <span v-else class="text-red-600">❌ {{ field.error }}</span>
          </div>
        </div>
      </div>

      <!-- 分类判断 -->
      <div v-if="result.matched" class="flex items-center gap-2">
        <span class="text-sm font-medium">分类判断:</span>
        <span class="text-sm text-gray-700">{{ result.category || '未分类' }}</span>
      </div>
    </div>

    <!-- 错误提示 -->
    <div v-if="errorMsg" class="mt-3 text-red-600 text-sm">{{ errorMsg }}</div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useTemplateStore } from '../stores/template'
import type { InvoiceTemplate, TestResult } from '../types'

const props = defineProps<{ template: InvoiceTemplate }>()
const templateStore = useTemplateStore()

const testPdfPath = ref('')
const testing = ref(false)
const result = ref<TestResult | null>(null)
const errorMsg = ref('')

async function selectTestPdf() {
  const selected = await open({
    filters: [{ name: 'PDF', extensions: ['pdf'] }],
    multiple: false,
  })
  if (selected && typeof selected === 'string') {
    testPdfPath.value = selected
    result.value = null
    errorMsg.value = ''
  }
}

async function runTest() {
  if (!testPdfPath.value) return
  testing.value = true
  result.value = null
  errorMsg.value = ''
  try {
    result.value = await templateStore.testTemplate(props.template, testPdfPath.value)
  } catch (e) {
    errorMsg.value = String(e)
  } finally {
    testing.value = false
  }
}
</script>
```

- [ ] **步骤 2：验证编译**

运行：`npx vue-tsc --noEmit`
预期：无错误

- [ ] **步骤 3：Commit**

```bash
git add src/components/TestPanel.vue
git commit -m "feat: 实现实时测试面板（选择PDF→运行测试→显示匹配/字段/分类结果）"
```

---

## 任务 9：端到端验证

- [ ] **步骤 1：前端类型检查**

运行：`npx vue-tsc --noEmit`
预期：无错误

- [ ] **步骤 2：后端编译**

运行：`cargo build --lib -p invoice-reimbursement`
预期：编译成功

- [ ] **步骤 3：后端全量测试**

运行：`cargo test --lib -p invoice-reimbursement -- --nocapture`
预期：所有测试 PASS

- [ ] **步骤 4：前端测试**

运行：`npm test`
预期：现有测试 PASS

- [ ] **步骤 5：启动应用手动验证**

运行：`npm run tauri dev`

手动验证清单：
1. 导航栏出现"模板"链接，点击进入模板列表页
2. 列表页显示内置模板（增值税普通发票等），内置模板有"复制"按钮无"编辑/删除"
3. 点"新建模板"→进入编辑器→填写名称/关键词→保存→返回列表页可见
4. 点内置模板"复制"→生成用户模板副本→可编辑
5. 编辑器手写模式：修改正则→切到标注模式→上传PDF→OCR文本显示→选择字段类型→拖选文本→正则自动生成→可编辑→确认→正则填入对应字段
6. 实时测试：选择PDF→开始测试→显示匹配结果和各字段提取结果
7. 启用/禁用模板开关可切换
8. 删除用户模板成功，删除内置模板无此按钮

- [ ] **步骤 6：Commit（如有修复）**

```bash
git add -A
git commit -m "fix: 端到端验证修复"
```

---

## 自检结果

### 规格覆盖度

| 规格章节 | 覆盖任务 |
|---------|---------|
| 4.1 模板列表页 | 任务 5 |
| 4.2 模板编辑器-手写模式 | 任务 6 |
| 4.2 模板编辑器-标注模式 | 任务 7 |
| 4.3 实时测试 | 任务 8 |
| 5.1 Tauri 命令（generate_regex_skeleton 补充） | 任务 2 |
| 6.1 错误处理（正则预编译、keywords 非空） | 任务 6（handleSave 验证）+ 后端任务 6 |
| 8. 实现顺序 Phase 3+4 | 任务 1-9 对应 Phase 3+4 |

### 占位符扫描

无占位符。所有步骤含完整代码。

### 类型一致性

- `InvoiceTemplate`/`FieldDefinition`/`FieldStrategy` 在任务 1 定义，任务 3/5/6/7/8 使用一致
- `TemplateMeta`/`TestResult`/`FieldTestResult` 在任务 1 定义，任务 3/5/8 使用一致
- `FieldType` 枚举在任务 1 定义，任务 7 使用一致
- `FIELD_TYPE_LABELS`/`CATEGORY_OPTIONS` 在任务 1 定义，任务 6/7 使用一致
- 后端 `generate_regex_skeleton` 命令参数 `field_type: String` 与前端 `FieldType` 枚举字符串值对应（任务 2/7）

### 已知限制

- 标注模式的文本拖选用 `window.getSelection()`，对复杂 OCR 文本（含换行）可能选到多余空白，后端 `generate_regex` 已做 trim 处理
- `category_keywords` 的响应式更新通过 `reactive` + 直接赋值实现，Vue 3 对 Record 类型的响应式追踪有限，极端情况下可能需 `Object.assign` 重建
- 导入/导出模板 JSON 功能（规格 4.1 提及）未在本计划中实现，属 YAGNI 范围，可后续按需添加

---

## 执行交接

两个计划已完成并保存：
- 后端：`docs/superpowers/plans/2026-06-23-configurable-matching-backend.md`（8 个任务）
- 前端：`docs/superpowers/plans/2026-06-23-configurable-matching-frontend.md`（9 个任务）

两种执行方式：

**1. 子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代

**2. 内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点

选哪种方式？
