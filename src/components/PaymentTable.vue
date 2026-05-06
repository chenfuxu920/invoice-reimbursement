<template>
  <div class="overflow-x-auto">
    <table class="w-full text-sm">
      <thead>
        <tr class="bg-gray-50 border-b">
          <th class="px-3 py-2 text-left font-medium text-gray-600">时间</th>
          <th class="px-3 py-2 text-left font-medium text-gray-600">商户</th>
          <th class="px-3 py-2 text-right font-medium text-gray-600">金额</th>
          <th class="px-3 py-2 text-left font-medium text-gray-600">来源</th>
          <th class="px-3 py-2 text-center font-medium text-gray-600">操作</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="p in payments" :key="p.id" class="border-b hover:bg-gray-50">
          <td class="px-3 py-2">{{ p.transaction_time }}</td>
          <td class="px-3 py-2">{{ p.merchant_name }}</td>
          <td class="px-3 py-2 text-right font-medium">¥{{ p.amount.toFixed(2) }}</td>
          <td class="px-3 py-2">
            <span :class="p.source === 'Wechat' ? 'text-green-600' : 'text-blue-600'">
              {{ p.source === 'Wechat' ? '微信' : '支付宝' }}
            </span>
          </td>
          <td class="px-3 py-2 text-center">
            <button @click="$emit('remove', p.id)" class="text-gray-400 hover:text-red-500">✕</button>
          </td>
        </tr>
      </tbody>
    </table>
    <div v-if="!payments.length" class="text-center py-8 text-gray-400">
      暂无支付记录
    </div>
  </div>
</template>

<script setup lang="ts">
import type { PaymentRecord } from '../types'

defineProps<{ payments: PaymentRecord[] }>()
defineEmits<{ (e: 'remove', id: string): void }>()
</script>
