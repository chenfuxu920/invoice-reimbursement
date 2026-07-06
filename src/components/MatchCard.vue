<template>
  <div class="bg-white rounded-lg border p-4 shadow-sm">
    <div class="flex justify-between items-start mb-3">
      <div class="flex items-center gap-2">
        <span class="px-2 py-0.5 rounded text-xs font-medium"
              :class="matchTypeClass">
          {{ matchTypeLabel }}
        </span>
        <span class="px-2 py-0.5 rounded text-xs"
              :class="confidenceClass">
          {{ (match.confidence * 100).toFixed(0) }}%
        </span>
      </div>
      <button @click="$emit('adjust', match)" class="text-sm text-blue-500 hover:text-blue-700">
        调整
      </button>
    </div>

    <div class="grid grid-cols-2 gap-3">
      <div class="bg-gray-50 rounded p-2 cursor-pointer hover:bg-gray-100 transition-colors" @click="$emit('view-invoice', match.invoice)">
        <div class="flex items-center justify-between">
          <p class="text-xs text-gray-500">发票 <span class="text-blue-500">查看详情 →</span></p>
          <select :value="match.invoice.category" @change="$emit('update-category', match.invoice.id, ($event.target as HTMLSelectElement).value as InvoiceCategory)" @click.stop
                  class="px-1 py-0.5 rounded text-xs border-0 cursor-pointer"
                  :class="getCategoryBadgeClass(match.invoice.category)">
            <option v-for="(label, key) in CATEGORY_LABELS" :key="key" :value="key">{{ label }}</option>
          </select>
        </div>
        <p class="font-medium">{{ match.invoice.invoice_number || '无编号' }}</p>
        <p class="text-sm text-gray-600">¥{{ match.invoice.amount.toFixed(2) }}</p>
      </div>
      <div class="bg-gray-50 rounded p-2 space-y-1">
        <p class="text-xs text-gray-500">支付</p>
        <!-- 行程级配对展示 -->
        <template v-if="hasItineraries">
          <div v-for="row in itineraryRows" :key="row.idx"
               class="flex items-center justify-between gap-1 py-0.5 group cursor-pointer hover:bg-gray-100 rounded px-1 -mx-1"
               @click="$emit('view-payment', match)">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <span class="shrink-0 text-xs text-gray-400">#{{ row.idx + 1 }}</span>
                <p class="text-sm font-medium truncate">{{ row.payment?.merchant_name || '未配对' }}</p>
                <span v-if="row.payment" class="shrink-0 text-xs px-1 py-0.5 rounded"
                      :class="row.payment.source === 'Wechat' ? 'bg-green-100 text-green-700' : 'bg-blue-100 text-blue-700'">
                  {{ row.payment.source === 'Wechat' ? '微信' : '支付宝' }}
                </span>
              </div>
              <div class="flex items-center gap-2 text-xs">
                <span class="text-gray-500">行程 ¥{{ row.itin.amount.toFixed(2) }}</span>
                <span class="text-gray-300">→</span>
                <span v-if="row.payment" class="text-gray-700 font-medium">¥{{ row.payment.amount.toFixed(2) }}</span>
                <span v-if="row.payment" class="text-orange-400">差¥{{ Math.abs(row.payment.amount - row.itin.amount).toFixed(2) }}</span>
                <template v-if="row.payment && row.timeDiffLabel">
                  <span class="text-gray-300">|</span>
                  <span class="text-orange-400">时差{{ row.timeDiffLabel }}</span>
                </template>
              </div>
            </div>
            <button v-if="row.payment" @click.stop="$emit('remove-payment', match.invoice_id, row.payment.id)"
                    class="text-gray-300 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                    title="移除此支付">
              ✕
            </button>
          </div>
        </template>
        <!-- 普通支付列表 -->
        <template v-else>
          <div v-for="p in match.payments" :key="p.id"
               class="flex items-center justify-between gap-1 py-0.5 group cursor-pointer hover:bg-gray-100 rounded px-1 -mx-1"
               @click="$emit('view-payment', match)">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <p class="text-sm font-medium truncate">{{ p.merchant_name || '未知' }}</p>
                <span class="shrink-0 text-xs px-1 py-0.5 rounded"
                      :class="p.source === 'Wechat' ? 'bg-green-100 text-green-700' : 'bg-blue-100 text-blue-700'">
                  {{ p.source === 'Wechat' ? '微信' : '支付宝' }}
                </span>
              </div>
              <div class="flex items-center gap-2 text-xs">
                <span class="text-gray-700 font-medium">¥{{ p.amount.toFixed(2) }}</span>
                <template v-if="p.refund_amount > 0 || p.discount > 0">
                  <span class="text-gray-300">|</span>
                  <span v-if="p.refund_amount > 0" class="text-red-400">退款 ¥{{ p.refund_amount.toFixed(2) }}</span>
                  <span v-if="p.refund_amount > 0 && p.discount > 0" class="text-gray-300"> </span>
                  <span v-if="p.discount > 0" class="text-green-400">优惠 ¥{{ p.discount.toFixed(2) }}</span>
                </template>
                <span class="text-gray-300">|</span>
                <span class="text-gray-400">{{ formatTime(p.transaction_time) }}</span>
              </div>
            </div>
            <button @click.stop="$emit('remove-payment', match.invoice_id, p.id)"
                    class="text-gray-300 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                    title="移除此支付">
              ✕
            </button>
          </div>
        </template>
      </div>
    </div>

    <div v-if="match.amount_diff > 0.01" class="mt-2 text-xs text-orange-500">
      金额差异: ¥{{ match.amount_diff.toFixed(2) }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { Invoice, MatchResult, InvoiceCategory } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass } from '../utils/category'

const props = defineProps<{ match: MatchResult }>()
defineEmits<{
  (e: 'adjust', match: MatchResult): void
  (e: 'view-invoice', invoice: Invoice): void
  (e: 'view-payment', match: MatchResult): void
  (e: 'update-category', invoiceId: string, category: InvoiceCategory): void
  (e: 'remove-payment', invoiceId: string, paymentId: string): void
}>()

const hasItineraries = computed(() => props.match.invoice.itineraries.length > 0)

/// 行程-支付配对行：优先用 itinerary_payment_pairs，无配对时回退按索引
const itineraryRows = computed(() => {
  const itins = props.match.invoice.itineraries
  const pairs = props.match.itinerary_payment_pairs || []
  return itins.map((itin, idx) => {
    const pair = pairs.find(p => p.itinerary_index === idx)
    const payment = pair
      ? props.match.payments.find(p => p.id === pair.payment_id)
      : props.match.payments[idx]
    const timeDiffLabel = payment ? itineraryTimeDiffLabel(itin.date_time, payment.transaction_time) : ''
    return { itin, idx, payment, timeDiffLabel }
  })
})

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
    OneToOne: '一对一',
    OneToMany: '一对多',
    Unmatched: '未匹配',
    ManualConfirmed: '手动确认'
  }
  return map[props.match.match_type] || '其他'
})

const matchTypeClass = computed(() => {
  const map: Record<string, string> = {
    OneToOne: 'bg-green-100 text-green-700',
    OneToMany: 'bg-yellow-100 text-yellow-700',
    Unmatched: 'bg-gray-100 text-gray-700',
    ManualConfirmed: 'bg-blue-100 text-blue-700'
  }
  return map[props.match.match_type] || 'bg-gray-100 text-gray-700'
})

const confidenceClass = computed(() => {
  if (props.match.confidence >= 0.9) return 'bg-green-100 text-green-700'
  if (props.match.confidence >= 0.7) return 'bg-yellow-100 text-yellow-700'
  return 'bg-red-100 text-red-700'
})

function formatTime(t: string) {
  if (t.length >= 16) return t.slice(5, 16)
  if (t.length >= 10) return t.slice(5, 10)
  return t
}
</script>
