<template>
  <div class="bg-white rounded-lg border p-4 shadow-sm">
    <div class="flex justify-between items-start">
      <div>
        <span class="inline-block px-2 py-0.5 rounded text-xs font-medium"
              :class="categoryClass">
          {{ categoryLabel }}
        </span>
        <p class="text-lg font-bold mt-2">¥{{ invoice.amount.toFixed(2) }}</p>
        <p class="text-sm text-gray-500">{{ invoice.seller_name || '未知销售方' }}</p>
      </div>
      <button @click="$emit('remove', invoice.id)" class="text-gray-400 hover:text-red-500">✕</button>
    </div>
    <div class="mt-2 text-xs text-gray-400 flex gap-4">
      <span>发票号: {{ invoice.invoice_number || '无' }}</span>
      <span>日期: {{ invoice.date }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { Invoice } from '../types'

const props = defineProps<{ invoice: Invoice }>()
defineEmits<{ (e: 'remove', id: string): void }>()

const categoryLabel = computed(() => {
  const map: Record<string, string> = {
    Train: '火车', Flight: '飞机', TicketChange: '退改签',
    CityTransport: '市内交通', Hotel: '住宿', Meal: '餐补', Other: '其他'
  }
  return map[props.invoice.category] || '其他'
})

const categoryClass = computed(() => {
  const map: Record<string, string> = {
    Train: 'bg-green-100 text-green-700',
    Flight: 'bg-blue-100 text-blue-700',
    TicketChange: 'bg-orange-100 text-orange-700',
    CityTransport: 'bg-purple-100 text-purple-700',
    Hotel: 'bg-yellow-100 text-yellow-700',
    Meal: 'bg-red-100 text-red-700',
    Other: 'bg-gray-100 text-gray-700'
  }
  return map[props.invoice.category] || 'bg-gray-100 text-gray-700'
})
</script>
