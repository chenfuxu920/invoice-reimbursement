<template>
  <div v-if="visible" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white rounded-[10px] shadow-2xl w-[520px] max-h-[85vh] flex flex-col">
      <div class="flex items-center justify-between px-5 py-4 border-b border-gray-100 shrink-0">
        <div class="flex items-center gap-2">
          <h2 class="text-base font-semibold text-gray-800">支付详情</h2>
          <span v-if="payments.length === 1" class="px-2 py-0.5 rounded text-xs font-medium" :class="sourceBadgeClass">{{ sourceLabel }}</span>
        </div>
        <button class="text-gray-400 hover:text-gray-600" aria-label="关闭" @click="$emit('close')">
          <AppIcon name="x" :size="16" />
        </button>
      </div>

      <!-- Tabs for multiple payments -->
      <div v-if="payments.length > 1" class="flex gap-1 px-5 pt-3 pb-1 border-b border-gray-100 shrink-0 overflow-x-auto">
        <button
          v-for="(p, i) in payments"
          :key="p.id"
          @click="activeIndex = i"
          class="px-3 py-1.5 text-xs rounded-t whitespace-nowrap"
          :class="i === activeIndex ? 'bg-primary-50 text-primary-700 font-medium border border-b-white border-primary-200' : 'text-gray-500 hover:bg-gray-50'"
        >
          {{ p.merchant_name.slice(0, 8) }}
        </button>
      </div>

      <div class="flex-1 overflow-auto px-5 py-4">
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

      <div class="px-5 py-3 border-t border-gray-100 flex justify-end shrink-0">
        <AppButton @click="$emit('close')">关闭</AppButton>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue'
import type { PaymentRecord } from '../types'
import AppButton from './ui/AppButton.vue'
import AppIcon from './ui/AppIcon.vue'

const props = defineProps<{ visible: boolean; payments: PaymentRecord[] }>()
const emit = defineEmits<{ (e: 'close'): void }>()

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.visible) emit('close')
}

watch(() => props.visible, (v) => {
  if (v) window.addEventListener('keydown', onKeydown)
  else window.removeEventListener('keydown', onKeydown)
}, { immediate: true })

onUnmounted(() => window.removeEventListener('keydown', onKeydown))

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
    : 'bg-primary-100 text-primary-700'
})
const refundClass = computed(() => {
  if (!activePayment.value) return ''
  return activePayment.value.refund_amount > 0 ? 'text-red-500' : ''
})
const totalAmount = computed(() =>
  props.payments.reduce((s, p) => s + p.amount, 0).toFixed(2)
)
</script>
