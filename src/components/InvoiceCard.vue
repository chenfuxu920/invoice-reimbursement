<template>
  <div class="bg-white rounded-lg border p-4 shadow-sm">
    <div class="flex justify-between items-start">
      <div>
        <span class="inline-block px-2 py-0.5 rounded text-xs font-medium"
              :class="getCategoryBadgeClass(invoice.category)">
          {{ getCategoryIcon(invoice.category) }} {{ getCategoryStyle(invoice.category).label }}
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
import type { Invoice } from '../types'
import { getCategoryStyle, getCategoryBadgeClass, getCategoryIcon } from '../utils/category'

defineProps<{ invoice: Invoice }>()
defineEmits<{ (e: 'remove', id: string): void }>()
</script>
