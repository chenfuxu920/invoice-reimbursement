<template>
  <div class="max-w-4xl mx-auto">
    <LoadingOverlay :visible="isLoading" :message="loadingMessage" />
    <div class="flex justify-between items-center mb-6">
      <h2 class="text-2xl font-bold">导入发票与账单</h2>
      <div class="flex gap-2">
        <button v-if="invoiceStore.invoices.length || paymentStore.payments.length" @click="handleClearAll"
                class="px-4 py-2 rounded bg-gray-500 text-white hover:bg-gray-600 transition-colors text-sm font-medium">
          清空全部
        </button>
        <button @click="handleGlobalImport" :disabled="globalLoading"
                class="px-4 py-2 rounded bg-purple-600 text-white hover:bg-purple-700 disabled:opacity-50 transition-colors text-sm font-medium">
          📂 全局导入
        </button>
      </div>
    </div>

    <div class="mb-8">
      <div class="flex items-center justify-between mb-3">
        <h3 class="text-lg font-medium">发票上传</h3>
        <button @click="blankVisible = true"
                class="px-3 py-1.5 rounded bg-blue-500 text-white hover:bg-blue-600 transition-colors text-sm font-medium">
          ＋ 手动添加空发票
        </button>
      </div>
      <InvoiceDropZone :loading="invoiceStore.loading" @files-selected="handleInvoiceFiles" />
      <div v-if="invoiceStore.invoices.length" class="mt-4 grid gap-3">
        <InvoiceCard v-for="inv in invoiceStore.invoices" :key="inv.id" :invoice="inv"
                     @remove="invoiceStore.removeInvoice" @view-detail="openInvoiceDetail" />
      </div>

      <!-- 解析失败错误区 -->
      <div v-if="invoiceStore.parseErrors.length" class="mt-4 border border-red-200 rounded-lg bg-red-50 p-4">
        <h4 class="text-sm font-medium text-red-700 mb-2">解析失败（{{ invoiceStore.parseErrors.length }}）</h4>
        <div class="space-y-2">
          <div v-for="err in invoiceStore.parseErrors" :key="err.id"
               class="flex items-center justify-between bg-white rounded px-3 py-2 border border-red-100">
            <div class="flex-1 min-w-0">
              <p class="text-sm text-gray-700 truncate">{{ err.fileName }}</p>
              <p class="text-xs text-red-500 truncate">{{ err.message }}</p>
            </div>
            <div class="flex gap-2 shrink-0 ml-2">
              <button @click="openManualEntry(err)" class="text-xs px-2 py-1 rounded bg-blue-500 text-white hover:bg-blue-600">手动填写</button>
              <button @click="retryParseError(err)" :disabled="retryingIds.includes(err.id)"
                      class="text-xs px-2 py-1 rounded border hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed">
                {{ retryingIds.includes(err.id) ? '重试中...' : '重试' }}
              </button>
              <button @click="invoiceStore.removeParseError(err.id)" class="text-xs px-2 py-1 rounded text-gray-400 hover:text-red-500">✕</button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="border-t pt-8">
      <h3 class="text-lg font-medium mb-3">账单导入</h3>
      <BillImporter @import="handleBillImport" />
      <details class="group mt-3" :open="!paymentStore.payments.length">
        <summary class="text-xs text-gray-400 cursor-pointer hover:text-gray-600 transition-colors select-none list-none flex items-center gap-1.5">
          <svg class="w-3 h-3 transition-transform group-open:rotate-90" viewBox="0 0 12 12" fill="none">
            <path d="M4 2l4 4-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span>账单下载指引</span>
        </summary>
        <div class="mt-2 space-y-1.5 pl-4">
          <div class="flex items-center gap-2 text-xs text-gray-500">
            <span class="inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-medium leading-tight bg-green-100 text-green-700">微信</span>
            <span>微信支付 → 我的账单 → 账单明细 → 右上角 ··· → 下载账单 → 用于个人对账</span>
          </div>
          <div class="flex items-center gap-2 text-xs text-gray-500">
            <span class="inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-medium leading-tight bg-blue-100 text-blue-700">支付宝</span>
            <span>我的 → 账单 → 右上角 ··· → 开具交易流水证明 → 用于个人对账</span>
          </div>
        </div>
      </details>
      <PaymentTable v-if="paymentStore.payments.length" :payments="paymentStore.payments" @remove="paymentStore.removePayment" class="mt-4" />
    </div>

    <InvoiceDetailModal :visible="detailVisible" :invoice="selectedInvoice" @close="detailVisible = false" @save="handleDetailSave" />
    <ManualInvoiceEntryModal :visible="manualVisible" :file-path="manualEntryFile" :error-id="manualEntryErrorId"
                             @close="manualVisible = false" @save="handleManualSave" />
    <BlankInvoiceEntryModal :visible="blankVisible" @close="blankVisible = false" @save="handleBlankSave" />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useInvoiceStore } from '../stores/invoice'
import { usePaymentStore } from '../stores/payment'
import { useMatchStore } from '../stores/match'
import InvoiceDropZone from '../components/InvoiceDropZone.vue'
import InvoiceCard from '../components/InvoiceCard.vue'
import BillImporter from '../components/BillImporter.vue'
import PaymentTable from '../components/PaymentTable.vue'
import LoadingOverlay from '../components/LoadingOverlay.vue'
import InvoiceDetailModal from '../components/InvoiceDetailModal.vue'
import ManualInvoiceEntryModal from '../components/ManualInvoiceEntryModal.vue'
import BlankInvoiceEntryModal from '../components/BlankInvoiceEntryModal.vue'
import type { Invoice, ParseError } from '../types'
import { invoke } from '@tauri-apps/api/core'

const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()

// 弹窗状态
const detailVisible = ref(false)
const selectedInvoice = ref<Invoice | null>(null)
const manualVisible = ref(false)
const manualEntryFile = ref('')
const manualEntryErrorId = ref('')
const blankVisible = ref(false)
const retryingIds = ref<string[]>([])

const globalLoading = ref(false)
const billLoading = ref(false)

const isLoading = computed(() => globalLoading.value || invoiceStore.loading || billLoading.value)

const loadingMessage = computed(() => {
  if (globalLoading.value) return '正在批量导入发票与账单...'
  if (invoiceStore.loading) return '正在识别发票...'
  if (billLoading.value) return '正在解析账单...'
  return '处理中...'
})

async function handleInvoiceFiles(paths: string[]) {
  // 先通过 Rust 展开所有路径（目录递归收集、文件直接保留，过滤支持的类型）
  const resolved: string[] = await invoke('collect_files', { paths, extensions: ['pdf', 'jpg', 'jpeg', 'png'] })
  if (resolved.length === 0) return

  const pdfs = resolved.filter(p => p.toLowerCase().endsWith('.pdf'))
  const images = resolved.filter(p => !p.toLowerCase().endsWith('.pdf'))

  invoiceStore.loading = true
  const skipped: string[] = []
  try {
    // 批量处理图片（addInvoice 内部已做跨批次去重）
    for (const path of images) {
      try {
        const added = await invoiceStore.addInvoice(path, 'image')
        if (!added) {
          // 重复跳过，发票号未知此处无法精确记录，仅计数
          skipped.push('(图片发票)')
        }
      }
      catch (e) { console.error('添加发票失败:', e) }
    }
    // 批量处理 PDF
    if (pdfs.length > 0) {
      const result: { invoices: any[], errors: [string, string][], duplicates: string[] } = await invoke('batch_recognize', { filePaths: pdfs })
      const crossSkipped = invoiceStore.addInvoicesSkipDuplicates(result.invoices)
      skipped.push(...result.duplicates, ...crossSkipped)
      // 将解析失败项写入 store 错误区
      const errs: ParseError[] = result.errors.map(([name, msg], i) => ({
        id: `pdf-${Date.now()}-${i}`,
        filePath: name,
        fileName: name.replace(/\\/g, '/').split('/').pop() || name,
        message: msg,
      }))
      invoiceStore.addParseErrors(errs)
    }
  } catch (e) {
    console.error('批量识别失败:', e)
  } finally {
    invoiceStore.loading = false
  }
  notifyDuplicates(skipped)
}

async function handleBillImport(paths: string[], type: 'wechat' | 'alipay') {
  // 先展开目录，过滤账单文件类型
  const resolved: string[] = await invoke('collect_files', { paths, extensions: ['xlsx', 'xls', 'csv'] })
  if (resolved.length === 0) return

  billLoading.value = true
  try {
    for (const filePath of resolved) {
      if (type === 'wechat') {
        await paymentStore.importWechatBill(filePath)
      } else {
        await paymentStore.importAlipayBill(filePath)
      }
    }
  } catch (e) {
    console.error('导入账单失败:', e)
  } finally {
    billLoading.value = false
  }
}

async function handleGlobalImport() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({ directory: true, multiple: true, title: '选择数据文件夹（可多选）' })
  if (!selected) return

  const dirs = Array.isArray(selected) ? selected : [selected]

  globalLoading.value = true
  let totalInvoices = 0
  let totalPayments = 0
  let allErrors: [string, string][] = []
  const allSkipped: string[] = []

  try {
    for (const dir of dirs) {
      const result = await invoke<{
        invoices: any[]
        payments: any[]
        errors: [string, string][]
        duplicates: string[]
      }>('batch_global_import', { dirPath: dir })

      const crossSkipped = invoiceStore.addInvoicesSkipDuplicates(result.invoices)
      allSkipped.push(...result.duplicates, ...crossSkipped)
      for (const p of result.payments) {
        paymentStore.payments.push(p)
      }

      totalInvoices += result.invoices.length - crossSkipped.length
      totalPayments += result.payments.length
      allErrors.push(...result.errors)
      const errs: ParseError[] = result.errors.map(([name, msg], i) => ({
        id: `global-${Date.now()}-${i}`,
        filePath: name,
        fileName: name.replace(/\\/g, '/').split('/').pop() || name,
        message: msg,
      }))
      invoiceStore.addParseErrors(errs)
    }

    const errCount = allErrors.length
    const dupCount = allSkipped.length
    const dupLine = dupCount > 0 ? `\n跳过重复发票 ${dupCount} 张：${formatDupDetail(allSkipped)}` : ''
    if (errCount > 0) {
      const details = allErrors.slice(0, 5).map(([n, e]) => `${n}: ${e}`).join('\n')
      const more = errCount > 5 ? `\n...及其他 ${errCount - 5} 个文件` : ''
      alert(`全局导入完成。\n成功：发票 ${totalInvoices} 张，账单 ${totalPayments} 条\n失败：${errCount} 个文件\n${details}${more}${dupLine}`)
    } else {
      alert(`全局导入完成！\n共导入发票 ${totalInvoices} 张，账单 ${totalPayments} 条${dupLine}`)
    }
  } catch (e) {
    console.error('全局导入失败:', e)
    alert('全局导入失败: ' + e)
  } finally {
    globalLoading.value = false
  }
}

/// 去重提示：统计 + 明细（最多展示 5 个，超出折叠）
function notifyDuplicates(skipped: string[]) {
  if (skipped.length === 0) return
  alert(`已跳过 ${skipped.length} 张重复发票：\n${formatDupDetail(skipped)}`)
}

function formatDupDetail(skipped: string[]): string {
  const shown = skipped.slice(0, 5).join('、')
  const more = skipped.length > 5 ? `\n...及其他 ${skipped.length - 5} 张` : ''
  return shown + more
}

function handleClearAll() {
  invoiceStore.clearInvoices()
  paymentStore.clearPayments()
  matchStore.clearMatches()
}

function openInvoiceDetail(invoice: Invoice) {
  selectedInvoice.value = invoice
  detailVisible.value = true
}

function handleDetailSave(updated: Invoice) {
  invoiceStore.updateInvoice(updated)
  detailVisible.value = false
}

function openManualEntry(err: ParseError) {
  manualEntryFile.value = err.filePath
  manualEntryErrorId.value = err.id
  manualVisible.value = true
}

function handleManualSave(invoice: Invoice, errorId: string) {
  invoiceStore.addManualInvoice(invoice)
  invoiceStore.removeParseError(errorId)
  manualVisible.value = false
}

function handleBlankSave(invoice: Invoice) {
  invoiceStore.addManualInvoice(invoice)
  blankVisible.value = false
}

async function retryParseError(err: ParseError) {
  if (retryingIds.value.includes(err.id)) return
  retryingIds.value.push(err.id)
  const isImage = /\.(jpg|jpeg|png)$/i.test(err.filePath)
  try {
    if (isImage) {
      const added = await invoiceStore.addInvoice(err.filePath, 'image')
      if (added) {
        invoiceStore.removeParseError(err.id)
      } else {
        alert('该发票已存在，无需重复导入')
        invoiceStore.removeParseError(err.id)
      }
    } else {
      const result: { invoices: any[], errors: [string, string][], duplicates: string[] } =
        await invoke('batch_recognize', { filePaths: [err.filePath] })
      if (result.invoices.length > 0) {
        invoiceStore.addInvoicesSkipDuplicates(result.invoices)
        invoiceStore.removeParseError(err.id)
      } else if (result.errors.length === 0 && result.duplicates.length > 0) {
        invoiceStore.removeParseError(err.id)
      }
    }
  } catch (e) {
    console.error('重试解析失败:', e)
    alert('重试失败: ' + e)
  } finally {
    retryingIds.value = retryingIds.value.filter(id => id !== err.id)
  }
}
</script>
