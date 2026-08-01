<template>
  <div class="flex items-center gap-1.5">
    <LoadingOverlay :visible="loading" :message="loadingMessage" />
    <button @click="exportFormHtml" :disabled="disabled || loading" title="生成报销单 HTML"
            class="w-8 h-8 rounded border hover:bg-gray-100 flex items-center justify-center text-sm disabled:opacity-50 disabled:cursor-not-allowed">
      📄
    </button>
    <button @click="exportComparisonImagePdf" :disabled="disabled || loading" title="生成对照 PDF（含发票图片）"
            class="w-8 h-8 rounded border hover:bg-gray-100 flex items-center justify-center text-sm disabled:opacity-50 disabled:cursor-not-allowed">
      🖼️
    </button>
    <button @click="exportFormXlsx" :disabled="disabled || loading" title="生成报销单 Excel"
            class="w-8 h-8 rounded border hover:bg-gray-100 flex items-center justify-center text-sm disabled:opacity-50 disabled:cursor-not-allowed">
      📊
    </button>
    <button @click="exportComparisonXlsx" :disabled="disabled || loading" title="生成完整信息对照单"
            class="w-8 h-8 rounded border hover:bg-gray-100 flex items-center justify-center text-sm disabled:opacity-50 disabled:cursor-not-allowed">
      📋
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import LoadingOverlay from './LoadingOverlay.vue'
import type { MatchResult } from '../types'

const props = defineProps<{
  matchResults: MatchResult[]
  unmatchedInvoiceIds: string[]
  unmatchedPaymentIds: string[]
  formInfo: {
    name: string
    department: string
    destination: string
    travelStart: string
    travelEnd: string
    companions: number
    hotelLevel: string
  }
  disabled?: boolean
}>()

const loading = ref(false)
const loadingMessage = ref('')

// 默认文件名：区分出差地与出差时间，如 报销单_上海_20260520-20260522.html
function defaultFileName(prefix: string, ext: string): string {
  const dest = props.formInfo.destination || '未设置'
  const start = (props.formInfo.travelStart || '').replace(/-/g, '')
  const end = (props.formInfo.travelEnd || '').replace(/-/g, '')
  const time = start && end ? `${start}-${end}` : new Date().toISOString().slice(0, 10)
  return `${prefix}_${dest}_${time}.${ext}`
}

async function exportFormHtml() {
  loading.value = true
  loadingMessage.value = '正在生成报销单...'
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const outputPath = await save({
      defaultPath: defaultFileName('报销单', 'html'),
      filters: [{ name: 'HTML', extensions: ['html'] }]
    })
    if (!outputPath) return

    await invoke('generate_reimbursement_html', {
      matchResults: props.matchResults,
      name: props.formInfo.name,
      department: props.formInfo.department,
      destination: props.formInfo.destination,
      travelStart: props.formInfo.travelStart,
      travelEnd: props.formInfo.travelEnd,
      companions: props.formInfo.companions,
      hotelLevel: props.formInfo.hotelLevel,
      outputPath
    })
    alert('报销单 HTML 已生成！')
  } catch (e) {
    console.error('生成失败:', e)
    alert('生成失败: ' + e)
  } finally {
    loading.value = false
  }
}

async function exportComparisonImagePdf() {
  loading.value = true
  loadingMessage.value = '正在生成对照单...'
  try {
    let invoiceDir = ''
    for (const r of props.matchResults) {
      if (r.invoice.source.type === 'Pdf' && r.invoice.source.path) {
        invoiceDir = r.invoice.source.path
          .split(/[\\/]/).slice(0, -1).join('/')
        if (invoiceDir) break
      }
    }

    const { save } = await import('@tauri-apps/plugin-dialog')
    const outputPath = await save({
      defaultPath: defaultFileName('对照表含图片', 'pdf'),
      filters: [{ name: 'PDF', extensions: ['pdf'] }]
    })
    if (!outputPath) return

    await invoke('generate_comparison_image_pdf', {
      matchResults: props.matchResults,
      invoiceDir,
      outputPath,
      destination: props.formInfo.destination || null,
    })
    alert('对照 PDF（含发票图片）已生成！')
  } catch (e) {
    console.error('生成失败:', e)
    alert('生成失败: ' + e)
  } finally {
    loading.value = false
  }
}

async function exportFormXlsx() {
  loading.value = true
  loadingMessage.value = '正在生成 Excel 报销单...'
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const outputPath = await save({
      defaultPath: defaultFileName('报销单', 'xlsx'),
      filters: [{ name: 'Excel', extensions: ['xlsx'] }]
    })
    if (!outputPath) return

    await invoke('generate_reimbursement_xlsx', {
      matchResults: props.matchResults,
      name: props.formInfo.name,
      department: props.formInfo.department,
      destination: props.formInfo.destination,
      travelStart: props.formInfo.travelStart,
      travelEnd: props.formInfo.travelEnd,
      companions: props.formInfo.companions,
      hotelLevel: props.formInfo.hotelLevel,
      outputPath
    })
    alert('报销单 Excel 已生成！')
  } catch (e) {
    console.error('生成失败:', e)
    alert('生成失败: ' + e)
  } finally {
    loading.value = false
  }
}

async function exportComparisonXlsx() {
  loading.value = true
  loadingMessage.value = '正在生成完整信息对照单...'
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const outputPath = await save({
      defaultPath: defaultFileName('信息对照单', 'xlsx'),
      filters: [{ name: 'Excel', extensions: ['xlsx'] }]
    })
    if (!outputPath) return

    await invoke('generate_comparison_xlsx', {
      matchResults: props.matchResults,
      outputPath
    })
    alert('完整信息对照单已生成！')
  } catch (e) {
    console.error('生成失败:', e)
    alert('生成失败: ' + e)
  } finally {
    loading.value = false
  }
}
</script>
