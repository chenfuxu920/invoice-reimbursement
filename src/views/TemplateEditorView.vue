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
        <div v-for="(keywords, category) in (template.category_keywords || {})" :key="category" class="flex items-center gap-2">
          <select :value="category" @change="updateCategoryKey(category, ($event.target as HTMLSelectElement).value)"
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
import { CATEGORY_OPTIONS, type InvoiceTemplate } from '../types'
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
