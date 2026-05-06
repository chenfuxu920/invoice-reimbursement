<template>
  <div class="max-w-4xl mx-auto">
    <div class="flex justify-between items-center mb-6">
      <h2 class="text-2xl font-bold">匹配结果</h2>
      <button @click="runAutoMatch"
              :disabled="invoiceStore.invoices.length === 0 || paymentStore.payments.length === 0 || matchStore.loading"
              class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-50">
        {{ matchStore.loading ? '匹配中...' : '自动匹配' }}
      </button>
    </div>

    <!-- 匹配结果 -->
    <div v-if="matchStore.matches.length" class="mb-8">
      <h3 class="text-lg font-medium mb-3">已匹配 ({{ matchStore.matches.length }})</h3>
      <div class="grid gap-3">
        <MatchCard v-for="m in matchStore.matches" :key="m.invoice_id" :match="m" @adjust="handleAdjust" />
      </div>
    </div>

    <!-- 未匹配发票 -->
    <div v-if="matchStore.unmatchedInvoices.length" class="mb-8">
      <h3 class="text-lg font-medium mb-3 text-orange-600">未匹配发票 ({{ matchStore.unmatchedInvoices.length }})</h3>
      <div class="grid gap-2">
        <div v-for="inv in matchStore.unmatchedInvoices" :key="inv.id"
             class="bg-orange-50 border border-orange-200 rounded p-3 flex justify-between items-center">
          <div>
            <span class="font-medium">{{ inv.invoice_number || '无编号' }}</span>
            <span class="text-gray-500 ml-2">¥{{ inv.amount.toFixed(2) }}</span>
          </div>
          <button @click="startManualMatch(inv)" class="text-sm text-blue-500 hover:text-blue-700">手动匹配</button>
        </div>
      </div>
    </div>

    <!-- 未匹配支付 -->
    <div v-if="matchStore.unmatchedPayments.length" class="mb-8">
      <h3 class="text-lg font-medium mb-3 text-gray-500">未匹配支付 ({{ matchStore.unmatchedPayments.length }})</h3>
      <div class="text-sm text-gray-400">
        <div v-for="p in matchStore.unmatchedPayments" :key="p.id" class="py-1">
          {{ p.merchant_name }} · ¥{{ p.amount.toFixed(2) }} · {{ p.transaction_time }}
        </div>
      </div>
    </div>

    <!-- 空状态 -->
    <div v-if="!matchStore.matches.length && !matchStore.unmatchedInvoices.length" class="text-center py-12 text-gray-400">
      请先在导入页面添加发票和账单，然后点击自动匹配
    </div>

    <!-- 手动匹配对话框 -->
    <MatchAdjustDialog
      :visible="showAdjustDialog"
      :invoice="adjustingInvoice"
      :available-payments="matchStore.unmatchedPayments"
      @close="showAdjustDialog = false"
      @confirm="handleManualMatch"
    />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useInvoiceStore } from '../stores/invoice'
import { usePaymentStore } from '../stores/payment'
import { useMatchStore } from '../stores/match'
import MatchCard from '../components/MatchCard.vue'
import MatchAdjustDialog from '../components/MatchAdjustDialog.vue'
import type { Invoice, MatchResult } from '../types'

const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()

const showAdjustDialog = ref(false)
const adjustingInvoice = ref<Invoice | null>(null)

async function runAutoMatch() {
  await matchStore.autoMatch(invoiceStore.invoices, paymentStore.payments)
}

function handleAdjust(match: MatchResult) {
  adjustingInvoice.value = match.invoice
  showAdjustDialog.value = true
}

function startManualMatch(invoice: Invoice) {
  adjustingInvoice.value = invoice
  showAdjustDialog.value = true
}

async function handleManualMatch(invoice: Invoice, paymentIds: string[]) {
  const payments = matchStore.unmatchedPayments.filter(p => paymentIds.includes(p.id))
  await matchStore.manualMatch(invoice, payments)
  showAdjustDialog.value = false
}
</script>
