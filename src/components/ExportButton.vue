<template>
  <div class="flex items-center gap-1.5">
    <LoadingOverlay :visible="loading" :message="loadingMessage" />
    <AppButton v-if="showLabels" secondary size="sm" :disabled="disabled || loading" @click="exportFormHtml">
      <AppIcon name="doc" :size="14" />
      报销单 HTML
    </AppButton>
    <button v-else @click="exportFormHtml" :disabled="disabled || loading" title="生成报销单 HTML" aria-label="生成报销单 HTML"
            class="w-8 h-8 rounded-lg border border-gray-300 hover:bg-gray-50 flex items-center justify-center disabled:opacity-50 disabled:cursor-not-allowed">
      <AppIcon name="doc" :size="16" />
    </button>
    <AppButton v-if="showLabels" secondary size="sm" :disabled="disabled || loading" @click="exportComparisonImagePdf">
      <AppIcon name="image" :size="14" />
      对照 PDF
    </AppButton>
    <button v-else @click="exportComparisonImagePdf" :disabled="disabled || loading" title="生成对照 PDF（含发票图片）" aria-label="生成对照 PDF（含发票图片）"
            class="w-8 h-8 rounded-lg border border-gray-300 hover:bg-gray-50 flex items-center justify-center disabled:opacity-50 disabled:cursor-not-allowed">
      <AppIcon name="image" :size="16" />
    </button>
    <AppButton v-if="showLabels" secondary size="sm" :disabled="disabled || loading" @click="exportFormXlsx">
      <AppIcon name="table" :size="14" />
      报销单 Excel
    </AppButton>
    <button v-else @click="exportFormXlsx" :disabled="disabled || loading" title="生成报销单 Excel" aria-label="生成报销单 Excel"
            class="w-8 h-8 rounded-lg border border-gray-300 hover:bg-gray-50 flex items-center justify-center disabled:opacity-50 disabled:cursor-not-allowed">
      <AppIcon name="table" :size="16" />
    </button>
    <AppButton v-if="showLabels" secondary size="sm" :disabled="disabled || loading" @click="exportComparisonXlsx">
      <AppIcon name="clipboard" :size="14" />
      信息对照单
    </AppButton>
    <button v-else @click="exportComparisonXlsx" :disabled="disabled || loading" title="生成完整信息对照单" aria-label="生成完整信息对照单"
            class="w-8 h-8 rounded-lg border border-gray-300 hover:bg-gray-50 flex items-center justify-center disabled:opacity-50 disabled:cursor-not-allowed">
      <AppIcon name="clipboard" :size="16" />
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import AppButton from './ui/AppButton.vue'
import AppIcon from './ui/AppIcon.vue'
import LoadingOverlay from './LoadingOverlay.vue'
import { toast } from '../composables/toast'
import type { MatchResult, Trip } from '../types'

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
  /// 批量导出模式：提供 trips 时，每个文件按趟分别导出到所选目录
  trips?: Trip[]
  disabled?: boolean
  showLabels?: boolean
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

function tripFileName(prefix: string, ext: string, trip: Trip): string {
  const dest = trip.destination || '未设置'
  const start = (trip.travelStart || '').replace(/-/g, '')
  const end = (trip.travelEnd || '').replace(/-/g, '')
  const time = start && end ? `${start}-${end}` : new Date().toISOString().slice(0, 10)
  return `${prefix}_${dest}_${time}.${ext}`
}

function invoiceDirFrom(matches: MatchResult[]): string {
  for (const r of matches) {
    if (r.invoice.source.type === 'Pdf' && r.invoice.source.path) {
      return r.invoice.source.path.split(/[\\/]/).slice(0, -1).join('/')
    }
  }
  return ''
}

/// 批量模式：选一个目录，把每个 trip 导出为单独文件。
async function exportEachTrip(
  fn: (trip: Trip, dir: string) => Promise<void>,
  message: string,
) {
  if (!props.trips || props.trips.length === 0) return
  const { open } = await import('@tauri-apps/plugin-dialog')
  const dir = await open({ directory: true })
  if (typeof dir !== 'string') return
  loading.value = true
  loadingMessage.value = message
  try {
    for (const trip of props.trips) {
      await fn(trip, dir)
    }
    toast(`已导出 ${props.trips.length} 个文件到：${dir}`, 'success')
  } catch (e) {
    console.error('生成失败:', e)
    toast('生成失败: ' + e, 'error')
  } finally {
    loading.value = false
  }
}

function formArgs(trip: Trip) {
  return {
    matchResults: trip.matches,
    name: '',
    department: '',
    destination: trip.destination,
    travelStart: trip.travelStart,
    travelEnd: trip.travelEnd,
    companions: 0,
    hotelLevel: trip.hotelLevel,
  }
}

async function exportFormHtml() {
  if (props.trips?.length) {
    await exportEachTrip(async (trip, dir) => {
      await invoke('generate_reimbursement_html', {
        ...formArgs(trip),
        outputPath: `${dir}/${tripFileName('报销单', 'html', trip)}`,
      })
    }, '正在生成报销单...')
    return
  }
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
    toast('报销单 HTML 已生成！', 'success')
  } catch (e) {
    console.error('生成失败:', e)
    toast('生成失败: ' + e, 'error')
  } finally {
    loading.value = false
  }
}

async function exportComparisonImagePdf() {
  if (props.trips?.length) {
    await exportEachTrip(async (trip, dir) => {
      await invoke('generate_comparison_image_pdf', {
        matchResults: trip.matches,
        invoiceDir: invoiceDirFrom(trip.matches),
        outputPath: `${dir}/${tripFileName('对照表含图片', 'pdf', trip)}`,
        destination: trip.destination || null,
      })
    }, '正在生成对照单...')
    return
  }
  loading.value = true
  loadingMessage.value = '正在生成对照单...'
  try {
    const invoiceDir = invoiceDirFrom(props.matchResults)
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
    toast('对照 PDF（含发票图片）已生成！', 'success')
  } catch (e) {
    console.error('生成失败:', e)
    toast('生成失败: ' + e, 'error')
  } finally {
    loading.value = false
  }
}

async function exportFormXlsx() {
  if (props.trips?.length) {
    await exportEachTrip(async (trip, dir) => {
      await invoke('generate_reimbursement_xlsx', {
        ...formArgs(trip),
        outputPath: `${dir}/${tripFileName('报销单', 'xlsx', trip)}`,
      })
    }, '正在生成 Excel 报销单...')
    return
  }
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
    toast('报销单 Excel 已生成！', 'success')
  } catch (e) {
    console.error('生成失败:', e)
    toast('生成失败: ' + e, 'error')
  } finally {
    loading.value = false
  }
}

async function exportComparisonXlsx() {
  if (props.trips?.length) {
    await exportEachTrip(async (trip, dir) => {
      await invoke('generate_comparison_xlsx', {
        matchResults: trip.matches,
        outputPath: `${dir}/${tripFileName('信息对照单', 'xlsx', trip)}`,
      })
    }, '正在生成完整信息对照单...')
    return
  }
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
    toast('完整信息对照单已生成！', 'success')
  } catch (e) {
    console.error('生成失败:', e)
    toast('生成失败: ' + e, 'error')
  } finally {
    loading.value = false
  }
}
</script>
