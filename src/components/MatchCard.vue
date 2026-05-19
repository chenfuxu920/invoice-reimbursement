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
