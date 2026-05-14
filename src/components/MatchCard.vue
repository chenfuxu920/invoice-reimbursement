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
        <p class="text-xs text-gray-500">发票 <span class="text-blue-500">查看详情 →</span></p>
        <p class="font-medium">{{ match.invoice.invoice_number || '无编号' }}</p>
        <p class="text-sm text-gray-600">¥{{ match.invoice.amount.toFixed(2) }}</p>
      </div>
      <div class="bg-gray-50 rounded p-2 cursor-pointer hover:bg-gray-100 transition-colors" @click="$emit('view-payment', match)">
        <p class="text-xs text-gray-500">支付 <span class="text-blue-500">查看详情 →</span></p>
        <template v-if="match.payments.length === 1">
          <p class="font-medium">{{ match.payments[0].merchant_name || '未知' }}</p>
          <p class="text-sm text-gray-600">¥{{ match.payments[0].amount.toFixed(2) }}</p>
        </template>
        <template v-else>
          <p class="font-medium">{{ match.payments.length }} 笔支付</p>
          <p class="text-sm text-gray-600">合计 ¥{{ totalPaymentAmount }}</p>
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
import type { Invoice, MatchResult } from '../types'

const props = defineProps<{ match: MatchResult }>()
defineEmits<{
  (e: 'adjust', match: MatchResult): void
  (e: 'view-invoice', invoice: Invoice): void
  (e: 'view-payment', match: MatchResult): void
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

const totalPaymentAmount = computed(() =>
  props.match.payments.reduce((sum, p) => sum + p.amount, 0).toFixed(2)
)
</script>
