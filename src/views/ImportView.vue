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
      <h3 class="text-lg font-medium mb-3">发票上传</h3>
      <InvoiceDropZone :loading="invoiceStore.loading" @files-selected="handleInvoiceFiles" />
      <div v-if="invoiceStore.invoices.length" class="mt-4 grid gap-3">
        <InvoiceCard v-for="inv in invoiceStore.invoices" :key="inv.id" :invoice="inv" @remove="invoiceStore.removeInvoice" />
      </div>
    </div>

    <div class="border-t pt-8">
      <h3 class="text-lg font-medium mb-3">账单导入</h3>
      <BillImporter @import="handleBillImport" />
      <PaymentTable v-if="paymentStore.payments.length" :payments="paymentStore.payments" @remove="paymentStore.removePayment" class="mt-4" />
    </div>

  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useInvoiceStore } from '../stores/invoice'
import { usePaymentStore } from '../stores/payment'
import InvoiceDropZone from '../components/InvoiceDropZone.vue'
import InvoiceCard from '../components/InvoiceCard.vue'
import BillImporter from '../components/BillImporter.vue'
import PaymentTable from '../components/PaymentTable.vue'
import LoadingOverlay from '../components/LoadingOverlay.vue'
import { invoke } from '@tauri-apps/api/core'

const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()

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
  const pdfs = paths.filter(p => p.toLowerCase().endsWith('.pdf'))
  if (pdfs.length === 0) {
    for (const path of paths) {
      try { await invoiceStore.addInvoice(path, 'image') }
      catch (e) { console.error('添加发票失败:', e) }
    }
    return
  }
  try {
    const result: { invoices: any[], errors: [string, string][] } = await invoke('batch_recognize', { filePaths: pdfs })
    for (const inv of result.invoices) {
      invoiceStore.invoices.push(inv)
    }
    for (const [name, err] of result.errors) {
      console.error(`文件 ${name} 识别失败:`, err)
    }
  } catch (e) {
    console.error('批量识别失败:', e)
  }
}

async function handleBillImport(filePath: string, type: 'wechat' | 'alipay') {
  billLoading.value = true
  try {
    if (type === 'wechat') {
      await paymentStore.importWechatBill(filePath)
    } else {
      await paymentStore.importAlipayBill(filePath)
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

  try {
    for (const dir of dirs) {
      const result = await invoke<{
        invoices: any[]
        payments: any[]
        errors: [string, string][]
      }>('batch_global_import', { dirPath: dir })

      for (const inv of result.invoices) {
        invoiceStore.invoices.push(inv)
      }
      for (const p of result.payments) {
        paymentStore.payments.push(p)
      }

      totalInvoices += result.invoices.length
      totalPayments += result.payments.length
      allErrors.push(...result.errors)
    }

    const errCount = allErrors.length
    if (errCount > 0) {
      const details = allErrors.slice(0, 5).map(([n, e]) => `${n}: ${e}`).join('\n')
      const more = errCount > 5 ? `\n...及其他 ${errCount - 5} 个文件` : ''
      alert(`全局导入完成。\n成功：发票 ${totalInvoices} 张，账单 ${totalPayments} 条\n失败：${errCount} 个文件\n${details}${more}`)
    } else {
      alert(`全局导入完成！\n共导入发票 ${totalInvoices} 张，账单 ${totalPayments} 条`)
    }
  } catch (e) {
    console.error('全局导入失败:', e)
    alert('全局导入失败: ' + e)
  } finally {
    globalLoading.value = false
  }
}

function handleClearAll() {
  invoiceStore.clearInvoices()
  paymentStore.clearPayments()
}
</script>
