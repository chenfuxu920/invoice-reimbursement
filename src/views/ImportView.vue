<template>
  <div class="max-w-4xl mx-auto">
    <h2 class="text-2xl font-bold mb-6">导入发票与账单</h2>

    <div class="mb-8">
      <h3 class="text-lg font-medium mb-3">发票上传</h3>
      <InvoiceDropZone :loading="invoiceStore.loading" @files-selected="handleInvoiceFiles" />
      <div v-if="invoiceStore.invoices.length" class="mt-4 grid gap-3">
        <InvoiceCard v-for="inv in invoiceStore.invoices" :key="inv.id" :invoice="inv" @remove="invoiceStore.removeInvoice" />
      </div>
    </div>

    <div class="border-t pt-8">
      <h3 class="text-lg font-medium mb-3">账单导入</h3>
      <p class="text-gray-400">微信/支付宝账单导入功能开发中...</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useInvoiceStore } from '../stores/invoice'
import InvoiceDropZone from '../components/InvoiceDropZone.vue'
import InvoiceCard from '../components/InvoiceCard.vue'

const invoiceStore = useInvoiceStore()

async function handleInvoiceFiles(paths: string[]) {
  for (const path of paths) {
    const fileType = path.toLowerCase().endsWith('.pdf') ? 'pdf' : 'image'
    try {
      await invoiceStore.addInvoice(path, fileType)
    } catch (e) {
      console.error('添加发票失败:', e)
    }
  }
}
</script>
