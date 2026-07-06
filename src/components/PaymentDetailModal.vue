<template>
  <div v-if="visible" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white rounded-lg shadow-xl w-[520px] max-h-[85vh] flex flex-col">
      <div class="flex items-center justify-between p-4 border-b shrink-0">
        <div class="flex items-center gap-2">
          <h3 class="font-medium">支付详情</h3>
          <span v-if="payments.length === 1" class="px-2 py-0.5 rounded text-xs font-medium" :class="sourceBadgeClass">{{ sourceLabel }}</span>
        </div>
        <button @click="$emit('close')" class="text-gray-400 hover:text-gray-600 text-xl leading-none">&times;</button>
      </div>

      <!-- Tabs for multiple payments -->
      <div v-if="payments.length > 1" class="flex gap-1 px-4 pt-3 pb-1 border-b shrink-0 overflow-x-auto">
        <button
          v-for="(p, i) in payments"
          :key="p.id"
          @click="activeIndex = i"
          class="px-3 py-1.5 text-xs rounded-t whitespace-nowrap"
          :class="i === activeIndex ? 'bg-blue-50 text-blue-700 font-medium border border-b-white border-blue-200' : 'text-gray-500 hover:bg-gray-50'"
        >
          {{ p.merchant_name.slice(0, 8) }}
        </button>
      </div>

      <div class="p-4 overflow-y-auto flex-1">
        <div class="grid grid-cols-2 gap-3 mb-4">
          <div class="bg-gray-50 rounded p-3 col-span-2">
            <p class="text-xs text-gray-500">商户名称</p>
            <p class="font-medium text-lg">{{ activePayment?.merchant_name || '未知' }}</p>
            <span v-if="payments.length > 1" class="px-2 py-0.5 rounded text-xs font-medium" :class="sourceBadgeClass">{{ sourceLabel }}</span>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">交易单号</p>
            <p class="font-medium text-sm break-all">{{ activePayment?.transaction_id || '未知' }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">交易时间</p>
            <p class="font-medium">{{ activePayment?.transaction_time || '未知' }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">交易金额</p>
            <p class="font-medium">¥{{ activePayment?.amount.toFixed(2) }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">原始金额</p>
            <p class="font-medium">¥{{ activePayment?.original_amount.toFixed(2) }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">退款金额</p>
            <p class="font-medium" :class="refundClass">¥{{ activePayment?.refund_amount.toFixed(2) }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">优惠折扣</p>
            <p class="font-medium text-green-600">¥{{ activePayment?.discount.toFixed(2) }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">支付方式</p>
            <p class="font-medium">{{ activePayment?.payment_method || '未知' }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">交易分类</p>
            <p class="font-medium">{{ activePayment?.category || '未知' }}</p>
          </div>
        </div>
        <div v-if="payments.length > 1" class="bg-gray-100 rounded p-3 text-sm">
          <div class="flex justify-between text-gray-600">
            <span>共 {{ payments.length }} 笔支付</span>
            <span class="font-medium">合计 ¥{{ totalAmount }}</span>
          </div>
        </div>
      </div>

      <div class="p-4 border-t flex justify-end shrink-0">
        <button @click="$emit('close')" class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 text-sm">
          关闭
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { PaymentRecord } from '../types'

const props = defineProps<{ visible: boolean; payments: PaymentRecord[] }>()
defineEmits<{ (e: 'close'): void }>()

const activeIndex = ref(0)
const activePayment = computed(() =>
  props.payments[activeIndex.value] || null
)

const sourceLabel = computed(() => {
  if (!activePayment.value) return ''
  return activePayment.value.source === 'Wechat' ? '微信' : '支付宝'
})
const sourceBadgeClass = computed(() => {
  if (!activePayment.value) return ''
  return activePayment.value.source === 'Wechat'
    ? 'bg-green-100 text-green-700'
    : 'bg-blue-100 text-blue-700'
})
const refundClass = computed(() => {
  if (!activePayment.value) return ''
  return activePayment.value.refund_amount > 0 ? 'text-red-500' : ''
})
const totalAmount = computed(() =>
  props.payments.reduce((s, p) => s + p.amount, 0).toFixed(2)
)
</script>