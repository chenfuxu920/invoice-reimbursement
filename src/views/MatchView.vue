<template>
  <div class="max-w-4xl mx-auto">
    <LoadingOverlay :visible="matchStore.loading" message="正在匹配发票与账单..." />
    <div class="flex justify-between items-center mb-6">
      <h2 class="text-2xl font-bold">匹配结果</h2>
      <button @click="runAutoMatch"
              :disabled="invoiceStore.invoices.length === 0 || paymentStore.payments.length === 0 || matchStore.loading"
              class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-50">
        自动匹配
      </button>
    </div>

    <!-- 匹配结果 -->
    <div v-if="matchStore.matches.length" class="mb-8">
      <h3 class="text-lg font-medium mb-3">已匹配 ({{ matchStore.matches.length }})</h3>
      <div class="grid gap-3">
        <MatchCard
          v-for="m in matchStore.matches"
          :key="m.invoice_id"
          :match="m"
          @adjust="handleAdjust"
          @view-invoice="handleViewInvoice"
          @view-payment="handleViewPayment"
          @update-category="handleUpdateCategory"
          @remove-payment="handleRemovePayment"
        />
      </div>
    </div>

    <!-- 未匹配发票 -->
    <div v-if="matchStore.unmatchedInvoices.length" class="mb-8">
      <h3 class="text-lg font-medium mb-3 text-orange-600">未匹配发票 ({{ matchStore.unmatchedInvoices.length }})</h3>
      <div class="grid gap-2">
        <div v-for="inv in matchStore.unmatchedInvoices" :key="inv.id"
             class="bg-orange-50 border border-orange-200 rounded p-3 flex justify-between items-center cursor-pointer hover:bg-orange-100 transition-colors"
             @click="handleViewInvoice(inv)">
          <div class="flex items-center gap-2">
            <span class="font-medium">{{ inv.invoice_number || '无编号' }}</span>
            <span class="text-gray-500">¥{{ inv.amount.toFixed(2) }}</span>
            <select :value="inv.category" @change="handleUpdateCategory(inv.id, ($event.target as HTMLSelectElement).value as InvoiceCategory)" @click.stop
                    class="px-1 py-0.5 rounded text-xs border-0 cursor-pointer"
                    :class="getCategoryBadgeClass(inv.category)">
              <option v-for="(label, key) in CATEGORY_LABELS" :key="key" :value="key">{{ label }}</option>
            </select>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-xs text-blue-400">查看详情</span>
            <button @click.stop="startManualMatch(inv)" class="text-sm text-blue-500 hover:text-blue-700">手动匹配</button>
          </div>
        </div>
      </div>
    </div>

    <!-- 未匹配支付（默认折叠） -->
    <div v-if="matchStore.unmatchedPayments.length" class="mb-8">
      <div @click="showUnmatchedPayments = !showUnmatchedPayments" class="cursor-pointer select-none">
        <h3 class="text-lg font-medium mb-1 text-gray-500">
          <span class="inline-block transition-transform duration-200" :class="{ 'rotate-90': showUnmatchedPayments }">▶</span>
          未匹配支付 ({{ matchStore.unmatchedPayments.length }})
        </h3>
      </div>
      <div v-show="showUnmatchedPayments" class="ml-4 space-y-1">
        <div v-for="p in matchStore.unmatchedPayments" :key="p.id"
             class="py-2 px-3 rounded cursor-pointer hover:bg-gray-100 transition-colors"
             @click="handleViewSinglePayment(p)">
          <div class="flex justify-between items-center">
            <span class="text-sm font-medium text-gray-700">{{ p.merchant_name }}</span>
            <span class="text-sm text-gray-600">¥{{ p.amount.toFixed(2) }}</span>
          </div>
          <span class="text-xs text-gray-400">{{ p.transaction_time }} · {{ p.source === 'Wechat' ? '微信' : '支付宝' }}</span>
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
      :current-payments="adjustingMatch?.payments || []"
      :available-payments="matchStore.unmatchedPayments"
      @close="handleAdjustClose"
      @confirm="handleManualMatch"
    />

    <!-- 发票详情弹窗 -->
    <InvoiceDetailModal
      :visible="showInvoiceDetail"
      :invoice="viewingInvoice"
      @close="showInvoiceDetail = false"
    />

    <!-- 支付详情弹窗 -->
    <PaymentDetailModal
      :visible="showPaymentDetail"
      :payments="viewingPayments"
      @close="showPaymentDetail = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useInvoiceStore } from '../stores/invoice'
import { usePaymentStore } from '../stores/payment'
import { useMatchStore } from '../stores/match'
import LoadingOverlay from '../components/LoadingOverlay.vue'
import MatchCard from '../components/MatchCard.vue'
import MatchAdjustDialog from '../components/MatchAdjustDialog.vue'
import InvoiceDetailModal from '../components/InvoiceDetailModal.vue'
import PaymentDetailModal from '../components/PaymentDetailModal.vue'
import type { Invoice, MatchResult, PaymentRecord, InvoiceCategory } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass } from '../utils/category'

const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()

const showUnmatchedPayments = ref(false)
const showAdjustDialog = ref(false)
const adjustingInvoice = ref<Invoice | null>(null)
const adjustingMatch = ref<MatchResult | null>(null)

const showInvoiceDetail = ref(false)
const viewingInvoice = ref<Invoice | null>(null)
const showPaymentDetail = ref(false)
const viewingPayments = ref<PaymentRecord[]>([])

async function runAutoMatch() {
  await matchStore.autoMatch(invoiceStore.invoices, paymentStore.payments)
}

function handleAdjust(match: MatchResult) {
  adjustingMatch.value = match
  adjustingInvoice.value = match.invoice
  showAdjustDialog.value = true
}

function handleAdjustClose() {
  showAdjustDialog.value = false
  adjustingMatch.value = null
}

function startManualMatch(invoice: Invoice) {
  adjustingMatch.value = null
  adjustingInvoice.value = invoice
  showAdjustDialog.value = true
}

async function handleManualMatch(invoice: Invoice, paymentIds: string[]) {
  const allPayments = [...matchStore.unmatchedPayments]
  if (adjustingMatch.value) {
    allPayments.push(...adjustingMatch.value.payments)
  }
  const payments = allPayments.filter(p => paymentIds.includes(p.id))
  await matchStore.manualMatch(invoice, payments)
  showAdjustDialog.value = false
  adjustingMatch.value = null
}

function handleUpdateCategory(invoiceId: string, category: InvoiceCategory) {
  invoiceStore.updateCategory(invoiceId, category)
  matchStore.updateInvoiceCategory(invoiceId, category)
}

function handleRemovePayment(invoiceId: string, paymentId: string) {
  matchStore.removePayment(invoiceId, paymentId)
}

function handleViewInvoice(invoice: Invoice) {
  viewingInvoice.value = invoice
  showInvoiceDetail.value = true
}

function handleViewPayment(match: MatchResult) {
  if (match.payments.length > 0) {
    viewingPayments.value = match.payments
    showPaymentDetail.value = true
  }
}

function handleViewSinglePayment(payment: PaymentRecord) {
  viewingPayments.value = [payment]
  showPaymentDetail.value = true
}
</script>
