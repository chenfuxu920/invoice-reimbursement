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
