<template>
  <div class="card card-hover p-5">
    <!-- 头部：匹配类型 + 置信度 + 调整 -->
    <div class="flex items-center justify-between mb-4">
      <div class="flex items-center gap-2">
        <span class="chip border font-semibold" :class="matchTypeClass">
          <component :is="matchTypeIcon" :size="12" />
          {{ matchTypeLabel }}
        </span>
        <span class="chip border" :class="confidenceClass">
          <Gauge :size="12" /> {{ (match.confidence * 100).toFixed(0) }}%
        </span>
        <span v-if="match.amount_diff > 0.01" :class="['chip', amountDiffChipClass(match.amount_diff)]" :title="'发票与支付金额差异'">
          差 ¥{{ match.amount_diff.toFixed(2) }}
        </span>
      </div>
      <AppButton variant="soft" size="sm" @click="$emit('adjust', match)">
        <SlidersHorizontal :size="13" /> 调整
      </AppButton>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
      <!-- 发票侧 -->
      <div class="rounded-xl bg-gradient-to-br from-slate-50 to-primary-50/60 border border-slate-200/70 p-3.5 cursor-pointer hover:border-primary-300 hover:shadow-card transition-all"
           @click="$emit('view-invoice', match.invoice)">
        <div class="flex items-center justify-between gap-2 mb-2">
          <p class="text-xs text-slate-500 flex items-center gap-1">
            <Receipt :size="12" class="text-primary-600" />
            发票 <span class="text-primary-600">查看详情 →</span>
          </p>
          <select :value="match.invoice.category" @change="$emit('update-category', match.invoice.id, ($event.target as HTMLSelectElement).value as InvoiceCategory)" @click.stop
                  class="input-sm !w-auto !py-1 text-xs cursor-pointer"
                  :class="getCategoryBadgeClass(match.invoice.category)">
            <option v-for="(label, key) in CATEGORY_LABELS" :key="key" :value="key">{{ label }}</option>
          </select>
        </div>
        <p class="font-display text-lg font-bold text-slate-800 truncate">{{ match.invoice.invoice_number || '无编号' }}</p>
        <p class="text-xs text-slate-400 truncate mt-0.5">{{ match.invoice.seller_name || '未知销售方' }}</p>
        <p class="font-display text-xl font-extrabold text-slate-900 tabular-nums mt-1">¥{{ match.invoice.amount.toFixed(2) }}</p>
        <!-- 行程明细：与右侧支付条目按 #N 对应，行高/字号对齐 -->
        <div v-if="hasItineraries" class="mt-2 pt-2 border-t border-slate-200/60">
          <div v-for="row in itineraryRows" :key="row.idx" class="flex items-center gap-1.5 py-1.5">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <span class="shrink-0 text-xs text-slate-400">#{{ row.idx + 1 }}</span>
                <p class="text-sm font-medium truncate">{{ row.itin.provider || '未知' }}</p>
              </div>
              <div class="flex items-center gap-1.5 text-xs mt-0.5">
                <span class="text-slate-500 min-w-0 truncate">{{ row.itin.date_time }} | {{ row.itin.pickup }} → {{ row.itin.dropoff }}</span>
                <span class="text-slate-700 font-medium ml-auto shrink-0">¥{{ row.itin.amount.toFixed(2) }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 支付侧 -->
      <div class="rounded-xl bg-slate-50/80 border border-slate-200/70 p-3.5 flex flex-col">
        <p class="text-xs text-slate-500 mb-1.5 flex items-center gap-1">
          <Wallet :size="12" class="text-emerald-600" /> 支付
        </p>
        <!-- 总计面板：仅一对多显示；高度与左侧发票详情区一致（实测 92px），保证上下两段一一对应 -->
        <div v-if="match.match_type === 'OneToMany'" class="min-h-[92px] flex flex-col">
          <div class="flex-1 flex flex-col justify-between mt-2 rounded-xl bg-white/80 border border-slate-200/70 p-2.5">
            <div class="flex justify-between text-xs text-slate-500">
              <span>共 {{ match.payments.length }} 笔支付<template v-if="hasItineraries"> · {{ match.invoice.itineraries.length }} 条行程</template><template v-if="unpairedCount > 0"> · <span class="text-amber-600 font-medium">{{ unpairedCount }} 条未配对</span></template></span>
              <span :class="`${amountDiffClass(totalDiff)} font-medium`">差额 ¥{{ Math.abs(totalDiff).toFixed(2) }}</span>
            </div>
            <p class="font-display text-base font-bold text-slate-800 tabular-nums">支付合计 ¥{{ paymentTotal.toFixed(2) }}</p>
            <div v-if="hasItineraries" class="flex justify-between text-xs text-slate-500">
              <span>行程合计</span>
              <span class="font-medium text-slate-700 tabular-nums">¥{{ itineraryTotal.toFixed(2) }}</span>
            </div>
          </div>
        </div>
        <div class="flex-1 mt-2 pt-2 border-t border-slate-200/70">
        <!-- 行程级配对展示 -->
        <template v-if="hasItineraries">
          <div v-for="row in itineraryRows" :key="row.idx"
               class="flex items-center justify-between gap-1.5 py-1.5 group cursor-pointer hover:bg-white rounded-lg px-1.5 -mx-1.5 transition-colors"
               @click="$emit('view-payment', match, row.payment)">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <span class="shrink-0 text-xs text-slate-400">#{{ row.idx + 1 }}</span>
                <p v-if="row.payment" class="text-sm font-medium truncate">{{ row.payment.merchant_name || '未知' }}</p>
                <span v-else class="shrink-0 chip border !py-0 !px-1.5 bg-amber-50 text-amber-700 border-amber-200/70"
                      title="该行程未匹配到支付记录">未配对</span>
                <span v-if="row.payment" class="shrink-0 chip border !py-0 !px-1.5"
                      :class="row.payment.source === 'Wechat' ? 'bg-emerald-50 text-emerald-700 border-emerald-200/70' : 'bg-primary-50 text-primary-700 border-primary-200/70'">
                  {{ row.payment.source === 'Wechat' ? '微信' : '支付宝' }}
                </span>
              </div>
              <div class="flex items-center gap-1.5 text-xs mt-0.5 flex-wrap">
                <span v-if="row.payment" class="text-slate-700 font-medium">¥{{ row.payment.amount.toFixed(2) }}</span>
                <span v-if="row.payment" :class="amountDiffClass(row.amountDiff)">差¥{{ row.amountDiff.toFixed(2) }}</span>
                <template v-if="row.payment && row.timeDiffLabel">
                  <span class="text-slate-300">|</span>
                  <span class="text-orange-400">时差{{ row.timeDiffLabel }}</span>
                </template>
                <template v-if="row.payment">
                  <span class="text-slate-300">|</span>
                  <span class="text-slate-400">{{ formatTime(row.payment.transaction_time) }}</span>
                </template>
              </div>
            </div>
            <button v-if="row.payment" @click.stop="requestRemovePayment(row.payment)"
                    class="text-slate-300 hover:text-rose-500 opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                    :aria-label="'移除此支付'" title="移除此支付">
              <X :size="13" />
            </button>
          </div>
        </template>
        <!-- 普通支付列表 -->
        <template v-else>
          <div v-for="p in match.payments" :key="p.id"
               class="flex items-center justify-between gap-1.5 py-1.5 group cursor-pointer hover:bg-white rounded-lg px-1.5 -mx-1.5 transition-colors"
               @click="$emit('view-payment', match, p)">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <p class="text-sm font-medium truncate">{{ p.merchant_name || '未知' }}</p>
                <span class="shrink-0 chip border !py-0 !px-1.5"
                      :class="p.source === 'Wechat' ? 'bg-emerald-50 text-emerald-700 border-emerald-200/70' : 'bg-primary-50 text-primary-700 border-primary-200/70'">
                  {{ p.source === 'Wechat' ? '微信' : '支付宝' }}
                </span>
              </div>
              <div class="flex items-center gap-1.5 text-xs mt-0.5 flex-wrap">
                <span class="text-slate-700 font-medium">¥{{ p.amount.toFixed(2) }}</span>
                <span v-if="match.match_type === 'OneToOne'" :class="amountDiffClass(Math.abs(p.amount - match.invoice.amount))">差¥{{ Math.abs(p.amount - match.invoice.amount).toFixed(2) }}</span>
                <template v-if="p.refund_amount > 0 || p.discount > 0">
                  <span class="text-slate-300">|</span>
                  <span v-if="p.refund_amount > 0" class="text-rose-400">退款 ¥{{ p.refund_amount.toFixed(2) }}</span>
                  <span v-if="p.discount > 0" class="text-emerald-400">优惠 ¥{{ p.discount.toFixed(2) }}</span>
                </template>
                <span class="text-slate-300">|</span>
                <span class="text-slate-400">{{ formatTime(p.transaction_time) }}</span>
              </div>
            </div>
            <button @click.stop="requestRemovePayment(p)"
                    class="text-slate-300 hover:text-rose-500 opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                    :aria-label="'移除此支付'" title="移除此支付">
              <X :size="13" />
            </button>
          </div>
        </template>
        </div>
      </div>
    </div>

    <ConfirmDialog :visible="removeTarget !== null" title="移除支付" :message="removeMessage"
                   confirm-text="移除" @confirm="confirmRemovePayment" @cancel="removeTarget = null" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { Receipt, Wallet, X, Gauge, SlidersHorizontal, Link2, Layers, HandCoins, UserCheck } from 'lucide-vue-next'
import AppButton from './ui/AppButton.vue'
import ConfirmDialog from './ui/ConfirmDialog.vue'
import type { Invoice, MatchResult, InvoiceCategory, PaymentRecord } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass } from '../utils/category'
import { amountDiffClass, amountDiffChipClass } from '../utils/amountDiff'

const props = defineProps<{ match: MatchResult }>()
const emit = defineEmits<{
  (e: 'adjust', match: MatchResult): void
  (e: 'view-invoice', invoice: Invoice): void
  (e: 'view-payment', match: MatchResult, payment?: PaymentRecord): void
  (e: 'update-category', invoiceId: string, category: InvoiceCategory): void
  (e: 'remove-payment', invoiceId: string, paymentId: string): void
}>()

const removeTarget = ref<{ invoiceId: string; payment: PaymentRecord } | null>(null)
const removeMessage = computed(() =>
  removeTarget.value ? `确定将该笔 ¥${removeTarget.value.payment.amount.toFixed(2)} 支付移出匹配？移出后将回到未匹配列表。` : ''
)

function requestRemovePayment(payment: PaymentRecord) {
  removeTarget.value = { invoiceId: props.match.invoice_id, payment }
}

function confirmRemovePayment() {
  if (removeTarget.value) {
    emit('remove-payment', removeTarget.value.invoiceId, removeTarget.value.payment.id)
  }
  removeTarget.value = null
}

const hasItineraries = computed(() => props.match.invoice.itineraries.length > 0)

const paymentTotal = computed(() => props.match.payments.reduce((s, p) => s + p.amount, 0))
const itineraryTotal = computed(() => props.match.invoice.itineraries.reduce((s, it) => s + it.amount, 0))
const totalDiff = computed(() => paymentTotal.value - itineraryTotal.value)

/// 行程-支付配对行：优先用 itinerary_payment_pairs；部分配对时未配上的行程
/// 不回退按索引取支付（避免错把他人支付展示到该行程上），仅旧数据（无配对）回退索引
const itineraryRows = computed(() => {
  const itins = props.match.invoice.itineraries
  const pairs = props.match.itinerary_payment_pairs || []
  return itins.map((itin, idx) => {
    const pair = pairs.find(p => p.itinerary_index === idx)
    const payment = pair
      ? props.match.payments.find(p => p.id === pair.payment_id)
      : pairs.length > 0
        ? undefined
        : props.match.payments[idx]
    const timeDiffLabel = payment ? itineraryTimeDiffLabel(itin.date_time, payment.transaction_time) : ''
    const amountDiff = payment ? Math.abs(payment.amount - itin.amount) : 0
    return { itin, idx, payment, amountDiff, timeDiffLabel }
  })
})

const unpairedCount = computed(() => itineraryRows.value.filter(r => !r.payment).length)

/// 行程时间与支付时间差异标签（具体时长）
function itineraryTimeDiffLabel(itinTime: string, payTime: string): string {
  const it = parseTimeToMs(itinTime)
  const pt = parseTimeToMs(payTime)
  if (it == null || pt == null) return '未知'
  return formatDuration(Math.abs(pt - it))
}

function parseTimeToMs(t: string): number | null {
  const s = t.trim().replace(/:+$/, '').trim()
  if (s.length >= 10 && s[4] === '-') {
    const d = new Date(s.slice(0, 16).replace(' ', 'T'))
    if (!isNaN(d.getTime())) return d.getTime()
    const d2 = new Date(s.slice(0, 10))
    return isNaN(d2.getTime()) ? null : d2.getTime()
  }
  if (s.length >= 5 && s[2] === '-' && /^\d{2}-\d{2}/.test(s)) {
    const invoiceYear = props.match.invoice.date ? props.match.invoice.date.slice(0, 4) : ''
    const candidates = invoiceYear
      ? [invoiceYear, String(new Date().getFullYear()), String(new Date().getFullYear() - 1)]
      : [String(new Date().getFullYear()), String(new Date().getFullYear() - 1)]
    for (const y of candidates) {
      const withYear = `${y}-${s}`
      const d = new Date(withYear.slice(0, 16).replace(' ', 'T'))
      if (!isNaN(d.getTime())) return d.getTime()
      const d2 = new Date(withYear.slice(0, 10))
      if (!isNaN(d2.getTime())) return d2.getTime()
    }
  }
  return null
}

function formatDuration(ms: number): string {
  const totalMin = Math.round(ms / (1000 * 60))
  if (totalMin < 1) return '0分钟'
  const days = Math.floor(totalMin / (60 * 24))
  const hours = Math.floor((totalMin % (60 * 24)) / 60)
  const mins = totalMin % 60
  const parts: string[] = []
  if (days > 0) parts.push(`${days}天`)
  if (hours > 0) parts.push(`${hours}小时`)
  if (mins > 0) parts.push(`${mins}分钟`)
  return parts.join('') || '0分钟'
}

const matchTypeLabel = computed(() => {
  const map: Record<string, string> = {
    OneToOne: '一对一', OneToMany: '一对多', Unmatched: '未匹配', ManualConfirmed: '手动确认',
  }
  return map[props.match.match_type] || '其他'
})

const matchTypeIcon = computed(() => {
  const map: Record<string, unknown> = {
    OneToOne: Link2, OneToMany: Layers, Unmatched: HandCoins, ManualConfirmed: UserCheck,
  }
  return map[props.match.match_type] || Link2
})

const matchTypeClass = computed(() => {
  const map: Record<string, string> = {
    OneToOne: 'bg-emerald-50 text-emerald-700 border-emerald-200/70',
    OneToMany: 'bg-amber-50 text-amber-700 border-amber-200/70',
    Unmatched: 'bg-slate-100 text-slate-600 border-slate-200/70',
    ManualConfirmed: 'bg-primary-50 text-primary-700 border-primary-200/70',
  }
  return map[props.match.match_type] || 'bg-slate-100 text-slate-600 border-slate-200/70'
})

const confidenceClass = computed(() => {
  if (props.match.confidence >= 0.9) return 'bg-emerald-50 text-emerald-700 border-emerald-200/70'
  if (props.match.confidence >= 0.7) return 'bg-amber-50 text-amber-700 border-amber-200/70'
  return 'bg-rose-50 text-rose-700 border-rose-200/70'
})

function formatTime(t: string) {
  if (t.length >= 16) return t.slice(5, 16)
  if (t.length >= 10) return t.slice(5, 10)
  return t
}
</script>
