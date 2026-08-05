<template>
  <div class="max-w-4xl mx-auto px-5 py-6 pb-8">
    <LoadingOverlay :visible="isLoading" :message="loadingMessage" />

    <!-- 页头 -->
    <div class="flex flex-wrap items-center justify-between gap-3 mb-6 animate-fade-in-up">
      <div>
        <h2 class="font-display text-2xl font-extrabold text-slate-900">收集票据</h2>
        <p class="text-sm text-slate-500 mt-1">拖入发票与账单，其余交给自动识别</p>
      </div>
      <div class="flex items-center gap-2">
        <AppButton variant="soft" size="sm" @click="blankVisible = true">
          <Plus :size="14" />
          手动添加空发票
        </AppButton>
        <AppButton variant="secondary" size="sm" :disabled="globalLoading" @click="handleGlobalImport">
          <FolderOpen :size="14" />
          全局导入
        </AppButton>
        <AppButton v-if="invoiceStore.invoices.length || paymentStore.payments.length" variant="ghost" size="sm"
                   class="text-rose-500 hover:bg-rose-50 hover:text-rose-600" @click="handleClearAll">
          <Trash2 :size="14" />
          清空全部
        </AppButton>
      </div>
    </div>

    <!-- 统一大拖拽区 -->
    <div class="animate-fade-in-up" style="animation-delay: 60ms">
      <InvoiceDropZone :loading="invoiceStore.loading" @files-selected="handleInvoiceFiles" @bills-import="handleBillImport" />
    </div>

    <!-- 发票卡片流 -->
    <div v-if="invoiceStore.invoices.length" class="mt-8 animate-fade-in-up" style="animation-delay: 120ms">
      <div class="flex items-center justify-between mb-3">
        <h3 class="font-display text-base font-bold text-slate-800 flex items-center gap-2">
          <Receipt :size="17" class="text-primary-600" />
          已识别发票（{{ invoiceStore.invoices.length }}）
        </h3>
        <span class="text-xs text-slate-400">点击卡片可展开行程明细</span>
      </div>
      <TransitionGroup name="card-flow" tag="div" class="grid gap-3">
        <InvoiceCard v-for="(inv, i) in invoiceStore.invoices" :key="inv.id" :invoice="inv"
                     :style="{ transitionDelay: `${Math.min(i * 40, 320)}ms` }"
                     @remove="invoiceStore.removeInvoice" @view-detail="openInvoiceDetail" />
      </TransitionGroup>
    </div>

    <!-- 解析失败错误区（醒目橙色错误条） -->
    <div v-if="invoiceStore.parseErrors.length" class="mt-8 animate-fade-in-up" style="animation-delay: 180ms">
      <div class="flex items-center gap-2 mb-3">
        <AlertTriangle :size="16" class="text-amber-500" />
        <h3 class="font-display text-base font-bold text-amber-700">有 {{ invoiceStore.parseErrors.length }} 张票据未能识别</h3>
        <p class="text-xs text-amber-500/80">可重试或手动补录</p>
      </div>
      <div class="space-y-2">
        <div v-for="err in invoiceStore.parseErrors" :key="err.id"
             class="group flex items-center justify-between gap-3 bg-gradient-to-r from-amber-50 to-orange-50/60 border border-amber-200/80 rounded-2xl px-4 py-3 shadow-card animate-scale-in">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-amber-800 truncate">{{ err.fileName }}</p>
            <p class="text-xs text-amber-600/90 truncate mt-0.5">{{ err.message }}</p>
          </div>
          <div class="flex gap-2 shrink-0 ml-2">
            <AppButton variant="primary" size="sm" @click="openManualEntry(err)">手动填写</AppButton>
            <AppButton variant="secondary" size="sm" @click="retryParseError(err)" :disabled="retryingIds.includes(err.id)" :loading="retryingIds.includes(err.id)">
              {{ retryingIds.includes(err.id) ? '' : '重试' }}
            </AppButton>
            <button @click="invoiceStore.removeParseError(err.id)" class="w-7 h-7 rounded-lg flex items-center justify-center text-amber-400 hover:text-rose-500 hover:bg-rose-50 transition-colors" title="删除" aria-label="删除">
              <X :size="14" />
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- 账单导入 -->
    <div class="mt-10 border-t border-slate-200/70 pt-8 animate-fade-in-up" style="animation-delay: 240ms">
      <div class="flex items-center justify-between mb-3">
        <h3 class="font-display text-base font-bold text-slate-800 flex items-center gap-2">
          <Wallet :size="17" class="text-emerald-600" />
          支付账单（{{ paymentStore.payments.length }} 条）
        </h3>
        <details class="group">
          <summary class="text-xs text-slate-400 cursor-pointer hover:text-primary-600 transition-colors select-none list-none flex items-center gap-1.5">
            <ChevronRight :size="13" class="transition-transform duration-200 group-open:rotate-90" />
            账单下载指引
          </summary>
          <div class="mt-2 space-y-1.5 pl-1">
            <div class="flex items-center gap-2 text-xs text-slate-500">
              <span class="chip bg-emerald-50 text-emerald-700 border border-emerald-200/70">微信</span>
              <span>支付 → 账单 → 账单明细 → ··· → 下载账单 → 用于个人对账</span>
            </div>
            <div class="flex items-center gap-2 text-xs text-slate-500">
              <span class="chip bg-primary-50 text-primary-700 border border-primary-200/70">支付宝</span>
              <span>我的 → 账单 → ··· → 开具交易流水证明 → 用于个人对账</span>
            </div>
          </div>
        </details>
      </div>
      <BillImporter @import="handleBillImport" />
      <PaymentTable v-if="paymentStore.payments.length" :payments="paymentStore.payments" @remove="paymentStore.removePayment" class="mt-4 card overflow-hidden" />
    </div>

    <!-- 底部 sticky：去核对匹配 -->
    <div class="sticky bottom-4 mt-10 z-30">
      <div class="glass rounded-2xl shadow-card-lg border border-primary-200/50 px-5 py-4 flex items-center justify-between gap-4">
        <div class="hidden sm:flex items-center gap-5 text-sm">
          <span class="text-slate-600"><b class="text-lg text-primary-700 tabular-nums">{{ invoiceStore.invoices.length }}</b> 张发票</span>
          <span class="text-slate-600"><b class="text-lg text-emerald-600 tabular-nums">{{ paymentStore.payments.length }}</b> 条账单</span>
          <span v-if="invoiceStore.parseErrors.length" class="text-amber-600"><b class="text-lg tabular-nums">{{ invoiceStore.parseErrors.length }}</b> 个待处理</span>
        </div>
        <button class="btn-primary-glow px-6 py-2.5 text-sm shrink-0" :disabled="!canGoMatch" @click="$router.push('/match')">
          去核对匹配
          <ArrowRight :size="16" />
        </button>
      </div>
    </div>

    <!-- 弹窗 -->
    <InvoiceDetailModal :visible="detailVisible" :invoice="selectedInvoice" @close="detailVisible = false" @save="handleDetailSave" />
    <ManualInvoiceEntryModal :visible="manualVisible" :file-path="manualEntryFile" :error-id="manualEntryErrorId"
                             @close="manualVisible = false" @save="handleManualSave" />
    <BlankInvoiceEntryModal :visible="blankVisible" @close="blankVisible = false" @save="handleBlankSave" />
    <ConfirmDialog :visible="clearConfirmVisible" title="清空全部数据"
                   message="确定清空全部发票、账单与匹配数据？此操作不可撤销。"
                   confirm-text="清空" @confirm="doClearAll" @cancel="clearConfirmVisible = false" />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import {
  Plus, FolderOpen, Trash2, Receipt, Wallet, AlertTriangle, X, ArrowRight, ChevronRight,
} from 'lucide-vue-next'
import { useInvoiceStore } from '../stores/invoice'
import { usePaymentStore } from '../stores/payment'
import { useMatchStore } from '../stores/match'
import InvoiceDropZone from '../components/InvoiceDropZone.vue'
import InvoiceCard from '../components/InvoiceCard.vue'
import BillImporter from '../components/BillImporter.vue'
import PaymentTable from '../components/PaymentTable.vue'
import LoadingOverlay from '../components/LoadingOverlay.vue'
import AppButton from '../components/ui/AppButton.vue'
import { toast } from '../composables/toast'
import InvoiceDetailModal from '../components/InvoiceDetailModal.vue'
import ManualInvoiceEntryModal from '../components/ManualInvoiceEntryModal.vue'
import BlankInvoiceEntryModal from '../components/BlankInvoiceEntryModal.vue'
import ConfirmDialog from '../components/ui/ConfirmDialog.vue'
import type { Invoice, ParseError } from '../types'
import { invoke } from '@tauri-apps/api/core'
import { consumePendingDrop } from '../composables/pendingDrop'

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
const clearConfirmVisible = ref(false)
const retryingIds = ref<string[]>([])

// 首页拖入的文件：消费暂存并直接执行识别（文件夹由 Rust collect_files 递归展开）
onMounted(() => {
  const drop = consumePendingDrop()
  if (!drop) return
  if (drop.invoices.length) handleInvoiceFiles(drop.invoices)
  if (drop.bills.length) handleBillImport(drop.bills)
})

const globalLoading = ref(false)
const billLoading = ref(false)

const isLoading = computed(() => globalLoading.value || invoiceStore.loading || billLoading.value)

const loadingMessage = computed(() => {
  if (globalLoading.value) return '正在批量导入发票与账单...'
  if (invoiceStore.loading) return '正在识别发票...'
  if (billLoading.value) return '正在解析账单...'
  return '处理中...'
})

const canGoMatch = computed(() => invoiceStore.invoices.length > 0 && paymentStore.payments.length > 0)

async function handleInvoiceFiles(paths: string[]) {
  // 通过 Rust 展开所有路径（目录递归收集、文件直接保留），同时收集发票与账单文件
  const resolved: string[] = await invoke('collect_files', {
    paths,
    extensions: ['pdf', 'jpg', 'jpeg', 'png', 'xlsx', 'xls', 'csv'],
  })
  if (resolved.length === 0) return

  const bills = resolved.filter(p => /\.(xlsx|xls|csv)$/i.test(p))
  const invoiceFiles = resolved.filter(p => !/\.(xlsx|xls|csv)$/i.test(p))
  if (bills.length > 0) await handleBillImport(bills)

  const pdfs = invoiceFiles.filter(p => p.toLowerCase().endsWith('.pdf'))
  const images = invoiceFiles.filter(p => !p.toLowerCase().endsWith('.pdf'))

  invoiceStore.loading = true
  const skipped: string[] = []
  try {
    for (const path of images) {
      try {
        const added = await invoiceStore.addInvoice(path, 'image')
        if (!added) skipped.push('(图片发票)')
      }
      catch (e) { console.error('添加发票失败:', e) }
    }
    if (pdfs.length > 0) {
      const result: { invoices: any[], errors: [string, string][], duplicates: string[] } = await invoke('batch_recognize', { filePaths: pdfs })
      const crossSkipped = invoiceStore.addInvoicesSkipDuplicates(result.invoices)
      skipped.push(...result.duplicates, ...crossSkipped)
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

async function handleBillImport(paths: string[]) {
  const resolved: string[] = await invoke('collect_files', { paths, extensions: ['xlsx', 'xls', 'csv'] })
  if (resolved.length === 0) return

  billLoading.value = true
  try {
    for (const filePath of resolved) {
      await paymentStore.importBill(filePath)
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
      toast(`全局导入完成。成功：发票 ${totalInvoices} 张，账单 ${totalPayments} 条；失败 ${errCount} 个文件，详见下方错误区。${dupLine}`, 'error')
    } else {
      toast(`全局导入完成！\n共导入发票 ${totalInvoices} 张，账单 ${totalPayments} 条${dupLine}`, 'success')
    }
  } catch (e) {
    console.error('全局导入失败:', e)
    toast('全局导入失败: ' + e, 'error')
  } finally {
    globalLoading.value = false
  }
}

function notifyDuplicates(skipped: string[]) {
  if (skipped.length === 0) return
  toast(`已跳过 ${skipped.length} 张重复发票：\n${formatDupDetail(skipped)}`, 'info')
}

function formatDupDetail(skipped: string[]): string {
  const shown = skipped.slice(0, 5).join('、')
  const more = skipped.length > 5 ? `\n...及其他 ${skipped.length - 5} 张` : ''
  return shown + more
}

function handleClearAll() {
  clearConfirmVisible.value = true
}

function doClearAll() {
  clearConfirmVisible.value = false
  invoiceStore.clearInvoices()
  paymentStore.clearPayments()
  matchStore.clearMatches()
  toast('已清空全部数据', 'info')
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
        toast('该发票已存在，无需重复导入', 'info')
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
    toast('重试失败: ' + e, 'error')
  } finally {
    retryingIds.value = retryingIds.value.filter(id => id !== err.id)
  }
}
</script>

<style scoped>
.card-flow-enter-active {
  transition: all 0.4s cubic-bezier(0.22, 1, 0.36, 1);
}
.card-flow-leave-active {
  transition: all 0.2s ease;
  position: absolute;
  width: calc(100% - 2.5rem);
}
.card-flow-enter-from {
  opacity: 0;
  transform: translateY(18px) scale(0.97);
}
.card-flow-leave-to {
  opacity: 0;
  transform: scale(0.95);
}
.card-flow-move {
  transition: transform 0.35s ease;
}
</style>
