<template>
  <div class="space-y-3">
    <button @click="exportFormHtml" :disabled="disabled"
            class="w-full px-4 py-3 rounded bg-blue-500 text-white font-medium hover:bg-blue-600 disabled:opacity-50 transition-colors">
      📄 生成报销单 HTML
    </button>
    <button @click="exportComparisonImagePdf" :disabled="disabled"
            class="w-full px-4 py-3 rounded bg-orange-500 text-white font-medium hover:bg-orange-600 disabled:opacity-50 transition-colors">
      🖼️ 生成对照 PDF（含发票图片）
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

async function exportFormHtml() {
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
  }
}

async function exportComparisonImagePdf() {
  try {
    let invoiceDir = ''
    for (const r of props.matchResults) {
      if (r.invoice.source.type === 'Pdf' && r.invoice.source.path) {
        invoiceDir = r.invoice.source.path
          .split(/[\\/]/).slice(0, -1).join('/')
        if (invoiceDir) break
      }
    }
    if (!invoiceDir) {
      alert('未找到发票 PDF 文件路径，请确认发票是通过文件导入的')
      return
    }

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
    })
    alert('对照 PDF（含发票图片）已生成！')
  } catch (e) {
    console.error('生成失败:', e)
    alert('生成失败: ' + e)
  }
}
</script>
