<template>
  <div class="space-y-3">
    <button @click="exportFormPdf" :disabled="disabled"
            class="w-full px-4 py-3 rounded bg-blue-500 text-white font-medium hover:bg-blue-600 disabled:opacity-50 transition-colors">
      📄 生成报销表单 PDF
    </button>
    <button @click="exportComparisonPdf" :disabled="disabled"
            class="w-full px-4 py-3 rounded bg-green-500 text-white font-medium hover:bg-green-600 disabled:opacity-50 transition-colors">
      📊 生成发票-支付对照 PDF
    </button>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { MatchResult } from '../types'

const props = defineProps<{
  matchResults: MatchResult[]
  unmatchedInvoiceIds: string[]
  unmatchedPaymentIds: string[]
  formInfo: { name: string; department: string; travelStart: string; travelEnd: string; companions: number }
  disabled?: boolean
}>()

async function exportFormPdf() {
  try {
    const outputPath = await selectSavePath('报销表单')
    if (!outputPath) return
    await invoke('generate_form_pdf', {
      matchResults: props.matchResults,
      name: props.formInfo.name,
      department: props.formInfo.department,
      travelStart: props.formInfo.travelStart,
      travelEnd: props.formInfo.travelEnd,
      companions: props.formInfo.companions,
      outputPath
    })
    alert('报销表单 PDF 已生成！')
  } catch (e) {
    console.error('生成失败:', e)
    alert('生成失败: ' + e)
  }
}

async function exportComparisonPdf() {
  try {
    const outputPath = await selectSavePath('对照表')
    if (!outputPath) return
    await invoke('generate_comparison_pdf', {
      matchResults: props.matchResults,
      unmatchedInvoiceIds: props.unmatchedInvoiceIds,
      unmatchedPaymentIds: props.unmatchedPaymentIds,
      outputPath
    })
    alert('对照 PDF 已生成！')
  } catch (e) {
    console.error('生成失败:', e)
    alert('生成失败: ' + e)
  }
}

async function selectSavePath(prefix: string): Promise<string | null> {
  // 使用 Tauri dialog 选择保存路径
  // 如果 @tauri-apps/plugin-dialog 不可用，使用默认路径
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    return await save({
      defaultPath: `${prefix}_${new Date().toISOString().slice(0, 10)}.pdf`,
      filters: [{ name: 'PDF', extensions: ['pdf'] }]
    })
  } catch {
    // fallback: 返回默认路径
    return `~/${prefix}_${new Date().toISOString().slice(0, 10)}.pdf`
  }
}
</script>
