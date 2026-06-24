<template>
  <div class="space-y-3">
    <LoadingOverlay :visible="loading" :message="loadingMessage" />
    <button @click="exportFormHtml" :disabled="disabled || loading"
            class="w-full px-4 py-3 rounded bg-blue-500 text-white font-medium hover:bg-blue-600 disabled:opacity-50 transition-colors">
      📄 生成报销单 HTML
    </button>
    <button @click="exportComparisonImagePdf" :disabled="disabled || loading"
            class="w-full px-4 py-3 rounded bg-orange-500 text-white font-medium hover:bg-orange-600 disabled:opacity-50 transition-colors">
      🖼️ 生成对照 PDF（含发票图片）
    </button>
    <button @click="exportFormXlsx" :disabled="disabled || loading"
            class="w-full px-4 py-3 rounded bg-green-500 text-white font-medium hover:bg-green-600 disabled:opacity-50 transition-colors">
      📊 生成报销单 Excel
    </button>
    <button @click="exportComparisonXlsx" :disabled="disabled || loading"
            class="w-full px-4 py-3 rounded bg-purple-500 text-white font-medium hover:bg-purple-600 disabled:opacity-50 transition-colors">
      📋 生成完整信息对照单
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

async function exportFormHtml() {
  loading.value = true
  loadingMessage.value = '正在生成报销单...'
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const outputPath = await save({
      defaultPath: `报销单_${new Date().toISOString().slice(0, 10)}.html`,
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
    // invoiceDir 可能为空（例如全部为手动添加的空发票），此时行程单查找会自动跳过，
    // 不再阻断生成——对照单仍可正常输出空发票留白页。

    const { save } = await import('@tauri-apps/plugin-dialog')
    const outputPath = await save({
      defaultPath: `对照表含图片_${new Date().toISOString().slice(0, 10)}.pdf`,
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
      defaultPath: `报销单_${new Date().toISOString().slice(0, 10)}.xlsx`,
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
      defaultPath: `信息对照单_${new Date().toISOString().slice(0, 10)}.xlsx`,
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
