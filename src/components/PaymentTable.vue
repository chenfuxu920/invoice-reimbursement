<template>
  <div class="overflow-x-auto">
    <table class="w-full text-sm">
      <thead>
        <tr class="bg-gray-50 border-b">
          <th class="px-3 py-2 text-left font-medium text-gray-600 w-6"></th>
          <th class="px-3 py-2 text-left font-medium text-gray-600">时间</th>
          <th class="px-3 py-2 text-left font-medium text-gray-600">商户</th>
          <th class="px-3 py-2 text-right font-medium text-gray-600">金额</th>
          <th class="px-3 py-2 text-left font-medium text-gray-600">支付方式</th>
          <th class="px-3 py-2 text-left font-medium text-gray-600">来源</th>
          <th class="px-3 py-2 text-center font-medium text-gray-600">操作</th>
        </tr>
      </thead>
      <tbody>
        <template v-for="p in payments" :key="p.id">
          <tr class="border-b hover:bg-gray-50 cursor-pointer" @click="toggle(p.id)">
            <td class="px-3 py-2 text-gray-400 transition-transform" :class="{ 'rotate-180': expandedId === p.id }">▾</td>
            <td class="px-3 py-2">{{ p.transaction_time || '-' }}</td>
            <td class="px-3 py-2">{{ p.merchant_name || '-' }}</td>
            <td class="px-3 py-2 text-right font-medium">
              ¥{{ p.amount.toFixed(2) }}
              <span v-if="p.refund_amount > 0 || p.discount > 0" class="text-xs text-gray-400 font-normal">
                <br v-if="p.refund_amount > 0 && p.discount > 0" />
                <template v-if="p.refund_amount > 0 && p.discount > 0"><span class="text-red-400">退¥{{ p.refund_amount.toFixed(2) }}</span> <span class="text-green-400">优¥{{ p.discount.toFixed(2) }}</span></template>
                <template v-else-if="p.refund_amount > 0"><span class="text-red-400">退款 ¥{{ p.refund_amount.toFixed(2) }}</span></template>
                <template v-else-if="p.discount > 0"><span class="text-green-400">优惠 ¥{{ p.discount.toFixed(2) }}</span></template>
              </span>
            </td>
            <td class="px-3 py-2 text-gray-500">{{ p.payment_method || '-' }}</td>
            <td class="px-3 py-2">
              <span :class="p.source === 'Wechat' ? 'text-green-600' : 'text-blue-600'">
                {{ p.source === 'Wechat' ? '微信' : '支付宝' }}
              </span>
            </td>
            <td class="px-3 py-2 text-center">
              <button @click.stop="$emit('remove', p.id)" class="text-gray-400 hover:text-red-500">✕</button>
            </td>
          </tr>
          <tr v-if="expandedId === p.id">
            <td :colspan="7" class="px-6 py-3 bg-gray-50">
              <div class="grid grid-cols-2 md:grid-cols-3 gap-3 text-xs">
                <div><span class="text-gray-400">交易单号:</span> <span class="text-gray-700">{{ p.transaction_id || '-' }}</span></div>
                <div><span class="text-gray-400">交易时间:</span> <span class="text-gray-700">{{ p.transaction_time }}</span></div>
                <div><span class="text-gray-400">实付金额:</span> <span class="text-gray-700 font-medium">¥{{ p.amount.toFixed(2) }}</span></div>
                <div><span class="text-gray-400">原始金额:</span> <span class="text-gray-700">¥{{ p.original_amount.toFixed(2) }}</span></div>
                <div><span class="text-gray-400">退款金额:</span> <span class="text-gray-700">¥{{ p.refund_amount.toFixed(2) }}</span></div>
                <div><span class="text-gray-400">优惠金额:</span> <span class="text-gray-700">¥{{ p.discount.toFixed(2) }}</span></div>
                <div><span class="text-gray-400">商户名称:</span> <span class="text-gray-700">{{ p.merchant_name }}</span></div>
                <div><span class="text-gray-400">来源:</span> <span class="text-gray-700">{{ p.source === 'Wechat' ? '微信' : '支付宝' }}</span></div>
                <div><span class="text-gray-400">交易类型:</span> <span class="text-gray-700">{{ p.category || '-' }}</span></div>
                <div><span class="text-gray-400">支付方式:</span> <span class="text-gray-700">{{ p.payment_method || '-' }}</span></div>
              </div>
            </td>
          </tr>
        </template>
      </tbody>
    </table>
    <div v-if="!payments.length" class="text-center py-8 text-gray-400">
      暂无支付记录
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import type { PaymentRecord } from '../types'

defineProps<{ payments: PaymentRecord[] }>()
defineEmits<{ (e: 'remove', id: string): void }>()

const expandedId = ref<string | null>(null)

function toggle(id: string) {
  expandedId.value = expandedId.value === id ? null : id
}
</script>
