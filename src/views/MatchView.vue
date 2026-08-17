<template>
  <div class="max-w-4xl mx-auto px-5 py-6 pb-8">
    <LoadingOverlay :visible="matchStore.loading" :message="matchProgressText" :progress="matchProgressPercent" />

    <!-- 状态横幅 + 大按钮 -->
    <section class="relative overflow-hidden rounded-3xl bg-gradient-to-br from-primary-600 via-accent-500 to-flare-500 shadow-float p-7 md:p-8 animate-fade-in-up">
      <!-- 装饰 -->
      <div class="absolute -top-16 -right-10 w-56 h-56 rounded-full bg-white/10 blur-2xl pointer-events-none" />
      <div class="absolute -bottom-20 -left-10 w-64 h-64 rounded-full bg-white/5 blur-2xl pointer-events-none" />
      <div v-if="matchStore.loading" class="scan-sweep" />

      <div class="relative flex flex-wrap items-center justify-between gap-6">
        <div class="min-w-0">
          <div class="flex items-center gap-2 mb-2">
            <span class="w-9 h-9 rounded-xl bg-white/20 backdrop-blur flex items-center justify-center text-white">
              <ScanSearch :size="18" />
            </span>
            <h2 class="font-display text-xl md:text-2xl font-extrabold text-white">核对匹配</h2>
          </div>
          <div class="flex items-center gap-6 text-white/90 mt-2">
            <span class="text-sm md:text-base">已匹配 <b class="text-2xl md:text-3xl font-extrabold tabular-nums">{{ matchCount }}</b></span>
            <span class="w-px h-8 bg-white/25" />
            <span class="text-sm md:text-base">待处理 <b class="text-2xl md:text-3xl font-extrabold tabular-nums">{{ pendingCount }}</b></span>
            <span v-if="!canMatch" class="text-xs text-white/70">（需先收集发票与账单）</span>
          </div>
        </div>
        <div class="flex flex-wrap items-center justify-end gap-3">
          <button
            class="inline-flex items-center gap-2.5 rounded-2xl bg-white text-primary-700 font-bold px-7 py-3.5 text-base shadow-card-lg transition-all duration-300 hover:scale-[1.03] hover:shadow-[0_16px_40px_-10px_rgb(0_0_0_/_0.4)] active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
            :disabled="!canMatch || matchStore.loading" @click="runAutoMatch">
            <Loader2 v-if="matchStore.loading" :size="20" class="animate-spin" />
            <Sparkles v-else :size="20" />
            {{ matchProgress ? `正在匹配 ${matchProgress.index + 1}/${matchProgress.total}` : (matchStore.loading ? '正在扫描匹配...' : (hasMatched ? '重新自动匹配' : '开始自动匹配')) }}
          </button>
        </div>
      </div>
    </section>

    <!-- 已匹配 -->
    <section v-if="matchStore.matches.length" class="mt-8">
      <div class="flex items-center justify-between mb-3 animate-fade-in-up">
        <h3 class="font-display text-base font-bold text-slate-800 flex items-center gap-2">
          <CheckCircle2 :size="17" class="text-emerald-500" />
          已匹配（{{ matchStore.matches.length }}）
        </h3>
        <span class="text-xs text-slate-400">点击卡片可调整 / 查看详情</span>
      </div>
      <div class="grid gap-3">
        <MatchCard
          v-for="(m, i) in matchStore.matches"
          :key="m.invoice_id"
          :match="m"
          :style="{ animationDelay: `${i * 70}ms` }"
          class="animate-fade-in-up"
          @adjust="handleAdjust"
          @view-invoice="handleViewInvoice"
          @view-payment="handleViewPayment"
          @update-category="handleUpdateCategory"
          @remove-payment="handleRemovePayment"
        />
      </div>
    </section>

    <!-- 未匹配发票 -->
    <section v-if="matchStore.unmatchedInvoices.length" class="mt-8">
      <div class="flex items-center gap-2 mb-3 animate-fade-in-up">
        <AlertTriangle :size="16" class="text-amber-500" />
        <h3 class="font-display text-base font-bold text-amber-700">未匹配发票（{{ matchStore.unmatchedInvoices.length }}）</h3>
      </div>
      <div class="space-y-2">
        <div v-for="(inv, i) in matchStore.unmatchedInvoices" :key="inv.id"
             class="card card-hover flex flex-wrap items-center justify-between gap-3 px-4 py-3 cursor-pointer animate-fade-in-up"
             :style="{ animationDelay: `${i * 60}ms` }"
             @click="handleViewInvoice(inv)">
          <div class="flex items-center gap-3 min-w-0">
            <span class="w-9 h-9 rounded-xl flex items-center justify-center shrink-0"
                  :class="getCategoryIconWrap(inv.category)">
              <AppIcon :name="getCategoryIcon(inv.category)" :size="16" />
            </span>
            <div class="min-w-0">
              <p class="font-medium text-slate-800 truncate">{{ inv.invoice_number || '无编号' }}</p>
              <p class="text-xs text-slate-400 truncate">{{ inv.seller_name || '未知销售方' }}</p>
            </div>
          </div>
          <div class="flex items-center gap-3 shrink-0">
            <span class="font-bold text-slate-800 tabular-nums">¥{{ inv.amount.toFixed(2) }}</span>
            <select :value="inv.category" @change="handleUpdateCategory(inv.id, ($event.target as HTMLSelectElement).value as InvoiceCategory)" @click.stop
                    class="input-sm !w-auto !py-1 text-xs cursor-pointer"
                    :class="getCategoryBadgeClass(inv.category)">
              <option v-for="(label, key) in CATEGORY_LABELS" :key="key" :value="key">{{ label }}</option>
            </select>
            <AppButton variant="primary" size="sm" @click.stop="startManualMatch(inv)">
              <Wand2 :size="13" /> 手动匹配
            </AppButton>
            <span class="text-xs text-primary-600">详情</span>
          </div>
        </div>
      </div>
    </section>

    <!-- 未匹配支付 -->
    <section v-if="matchStore.unmatchedPayments.length" class="mt-8">
      <button @click="showUnmatchedPayments = !showUnmatchedPayments" class="cursor-pointer select-none text-left w-full animate-fade-in-up"
              :aria-expanded="showUnmatchedPayments">
        <div class="flex items-center gap-2">
          <ChevronRight :size="15" class="text-slate-400 transition-transform duration-200" :class="{ 'rotate-90': showUnmatchedPayments }" />
          <h3 class="font-display text-base font-bold text-slate-700">未匹配支付（{{ matchStore.unmatchedPayments.length }}）</h3>
          <span class="text-xs text-slate-400">点击查看详情</span>
        </div>
      </button>
      <div v-show="showUnmatchedPayments" class="ml-5 mt-3 space-y-2">
        <div v-for="p in matchStore.unmatchedPayments" :key="p.id"
             class="card card-hover px-4 py-3 flex items-center justify-between gap-3 cursor-pointer animate-fade-in-up"
             @click="handleViewSinglePayment(p)">
          <div class="flex items-center gap-3 min-w-0">
            <span class="w-8 h-8 rounded-lg flex items-center justify-center shrink-0"
                  :class="p.source === 'Wechat' ? 'bg-emerald-50 text-emerald-600' : 'bg-primary-50 text-primary-600'">
              <AppIcon :name="p.source === 'Wechat' ? 'table' : 'table'" :size="14" />
            </span>
            <div class="min-w-0">
              <p class="text-sm font-medium text-slate-700 truncate">{{ p.merchant_name }}</p>
              <span class="text-xs text-slate-400">{{ p.transaction_time }} · {{ p.source === 'Wechat' ? '微信' : '支付宝' }}</span>
            </div>
          </div>
          <span class="text-sm font-semibold text-slate-800 tabular-nums shrink-0">¥{{ p.amount.toFixed(2) }}</span>
        </div>
      </div>
    </section>

    <!-- 空状态 -->
    <AppEmpty v-if="!matchStore.matches.length && !matchStore.unmatchedInvoices.length && !matchStore.unmatchedPayments.length"
              icon="link" message="请先在收集票据页添加发票和账单，然后点击自动匹配" class="mt-4 animate-fade-in-up">
      <AppButton variant="primary" @click="$router.push('/import')">去收集票据</AppButton>
    </AppEmpty>

    <!-- 底部 sticky：去打包导出 -->
    <div v-if="matchStore.matches.length" class="sticky bottom-4 mt-10 z-30">
      <div class="glass rounded-2xl shadow-card-lg border border-primary-200/50 px-5 py-4 flex items-center justify-between gap-4">
        <div class="hidden sm:flex items-center gap-5 text-sm">
          <span class="text-slate-600"><b class="text-lg text-primary-700 tabular-nums">{{ matchStore.matches.length }}</b> 个已匹配</span>
          <span v-if="matchStore.unmatchedInvoices.length || matchStore.unmatchedPayments.length" class="text-amber-600">
            <b class="text-lg tabular-nums">{{ matchStore.unmatchedInvoices.length + matchStore.unmatchedPayments.length }}</b> 个待处理
          </span>
          <span v-else class="text-emerald-600"><CheckCircle2 :size="15" class="inline-block -mt-0.5 mr-1" />全部处理完毕</span>
        </div>
        <button class="btn-primary-glow px-6 py-2.5 text-sm shrink-0" @click="$router.push('/export')">
          去打包导出
          <ArrowRight :size="16" />
        </button>
      </div>
    </div>

    <!-- 手动匹配对话框 -->
    <MatchAdjustDialog
      :visible="showAdjustDialog"
      :invoice="adjustingInvoice"
      :current-payments="adjustingMatch?.payments || []"
      :current-pairs="adjustingMatch?.itinerary_payment_pairs || []"
      :available-payments="matchStore.unmatchedPayments"
      @close="handleAdjustClose"
      @confirm="handleManualMatch"
    />

    <!-- 发票详情弹窗 -->
    <InvoiceDetailModal
      :visible="showInvoiceDetail"
      :invoice="viewingInvoice"
      @close="showInvoiceDetail = false"
      @save="handleDetailSave"
    />

    <!-- 支付详情弹窗 -->
    <PaymentDetailModal
      :visible="showPaymentDetail"
      :payments="viewingPayments"
      :initial-index="viewingPaymentIndex"
      @close="showPaymentDetail = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import {
  ScanSearch, Sparkles, CheckCircle2, AlertTriangle, ArrowRight, ChevronRight, Wand2, Loader2,
} from 'lucide-vue-next'
import { useInvoiceStore } from '../stores/invoice'
import { usePaymentStore } from '../stores/payment'
import { useMatchStore } from '../stores/match'
import { useCountUp } from '../composables/useCountUp'
import LoadingOverlay from '../components/LoadingOverlay.vue'
import MatchCard from '../components/MatchCard.vue'
import AppButton from '../components/ui/AppButton.vue'
import AppIcon from '../components/ui/AppIcon.vue'
import AppEmpty from '../components/ui/AppEmpty.vue'
import { toast } from '../composables/toast'
import { listen } from '@tauri-apps/api/event'
import MatchAdjustDialog from '../components/MatchAdjustDialog.vue'
import InvoiceDetailModal from '../components/InvoiceDetailModal.vue'
import PaymentDetailModal from '../components/PaymentDetailModal.vue'
import type { Invoice, MatchResult, PaymentRecord, InvoiceCategory, ItineraryPaymentPair } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass, getCategoryIcon } from '../utils/category'

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
const viewingPaymentIndex = ref(0)

const canMatch = computed(() => invoiceStore.invoices.length > 0 && paymentStore.payments.length > 0)
const hasMatched = computed(() => matchStore.matches.length > 0)

const matchProgress = ref<{ index: number; total: number } | null>(null)
const matchProgressPercent = computed(() =>
  matchProgress.value ? Math.round((matchProgress.value.index + 1) / matchProgress.value.total * 100) : undefined
)
const matchProgressText = computed(() =>
  matchProgress.value ? `正在匹配发票与账单 ${matchProgress.value.index + 1}/${matchProgress.value.total}...` : '正在匹配发票与账单...'
)

let unlistenMatch: (() => void) | undefined
onMounted(async () => {
  unlistenMatch = await listen<{ index: number; total: number }>('match-progress', (e) => {
    matchProgress.value = e.payload
  })
})
onUnmounted(() => {
  unlistenMatch?.()
})

const matchCount = useCountUp(() => matchStore.matches.length)
const pendingCount = useCountUp(() => matchStore.unmatchedInvoices.length + matchStore.unmatchedPayments.length)

const ICON_WRAPS: Record<InvoiceCategory, string> = {
  Train: 'bg-emerald-100 text-emerald-600',
  Flight: 'bg-primary-100 text-primary-600',
  Insurance: 'bg-cyan-100 text-cyan-600',
  TicketChange: 'bg-amber-100 text-amber-600',
  CityTransport: 'bg-violet-100 text-violet-600',
  Hotel: 'bg-yellow-100 text-yellow-600',
  Meal: 'bg-rose-100 text-rose-600',
  Toll: 'bg-indigo-100 text-indigo-600',
  Other: 'bg-slate-100 text-slate-600',
}
function getCategoryIconWrap(category: InvoiceCategory) {
  return ICON_WRAPS[category] || ICON_WRAPS.Other
}

async function runAutoMatch() {
  matchProgress.value = null
  try {
    await matchStore.autoMatch(invoiceStore.invoices, paymentStore.payments)
  } catch (e) {
    toast('自动匹配失败: ' + e, 'error')
  } finally {
    // 匹配结束（成功或失败）都必须清掉进度，否则按钮会一直卡在“正在匹配 x/y”
    matchProgress.value = null
  }
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

async function handleManualMatch(invoice: Invoice, paymentIds: string[], itineraryPaymentPairs: ItineraryPaymentPair[] = []) {
  const allPayments = [...matchStore.unmatchedPayments]
  if (adjustingMatch.value) {
    allPayments.push(...adjustingMatch.value.payments)
  }
  const payments = allPayments.filter(p => paymentIds.includes(p.id))
  await matchStore.manualMatch(invoice, payments, itineraryPaymentPairs)
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

async function handleDetailSave(updated: Invoice) {
  invoiceStore.updateInvoice(updated)
  showInvoiceDetail.value = false
  // 自动重新匹配
  await matchStore.autoMatch(invoiceStore.invoices, paymentStore.payments)
}

function handleViewPayment(match: MatchResult, payment?: PaymentRecord) {
  if (match.payments.length > 0) {
    viewingPayments.value = match.payments
    viewingPaymentIndex.value = payment
      ? Math.max(0, match.payments.findIndex(p => p.id === payment.id))
      : 0
    showPaymentDetail.value = true
  }
}

function handleViewSinglePayment(payment: PaymentRecord) {
  viewingPayments.value = [payment]
  viewingPaymentIndex.value = 0
  showPaymentDetail.value = true
}
</script>
