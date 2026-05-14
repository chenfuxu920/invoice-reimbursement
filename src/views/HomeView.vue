<template>
  <div class="max-w-4xl mx-auto">
    <h2 class="text-2xl font-bold mb-6">欢迎使用发票报销助手 v0.1.0</h2>

    <!-- OCR 服务状态 -->
    <div class="bg-white rounded-lg border p-4 shadow-sm mb-6">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <span class="w-3 h-3 rounded-full" :class="ocrOnline ? 'bg-green-500' : 'bg-red-500'"></span>
          <span class="font-medium">OCR 识别服务</span>
        </div>
        <span class="text-sm" :class="ocrOnline ? 'text-green-600' : 'text-red-500'">
          {{ ocrOnline ? '在线' : '离线' }}
        </span>
      </div>
    </div>

    <!-- 数据统计卡片 -->
    <div class="grid grid-cols-3 gap-4 mb-6">
      <div class="bg-white rounded-lg border p-4 shadow-sm text-center">
        <p class="text-3xl font-bold text-blue-600">{{ invoiceStore.invoices.length }}</p>
        <p class="text-sm text-gray-500 mt-1">已导入发票</p>
      </div>
      <div class="bg-white rounded-lg border p-4 shadow-sm text-center">
        <p class="text-3xl font-bold text-green-600">{{ paymentStore.payments.length }}</p>
        <p class="text-sm text-gray-500 mt-1">支付记录</p>
      </div>
      <div class="bg-white rounded-lg border p-4 shadow-sm text-center">
        <p class="text-3xl font-bold text-purple-600">{{ matchStore.matches.length }}</p>
        <p class="text-sm text-gray-500 mt-1">已匹配</p>
      </div>
    </div>

    <!-- 快速操作 -->
    <div class="bg-white rounded-lg border p-5 shadow-sm">
      <h3 class="font-medium text-lg mb-4">快速操作</h3>
      <div class="grid grid-cols-3 gap-4">
        <router-link to="/import"
          class="flex flex-col items-center gap-2 p-4 rounded-lg border-2 border-dashed border-gray-200 hover:border-blue-400 hover:bg-blue-50 transition-colors">
          <span class="text-2xl">📤</span>
          <span class="text-sm font-medium">导入发票</span>
        </router-link>
        <router-link to="/import"
          class="flex flex-col items-center gap-2 p-4 rounded-lg border-2 border-dashed border-gray-200 hover:border-green-400 hover:bg-green-50 transition-colors">
          <span class="text-2xl">💳</span>
          <span class="text-sm font-medium">导入账单</span>
        </router-link>
        <router-link to="/match"
          class="flex flex-col items-center gap-2 p-4 rounded-lg border-2 border-dashed border-gray-200 hover:border-purple-400 hover:bg-purple-50 transition-colors">
          <span class="text-2xl">🔗</span>
          <span class="text-sm font-medium">开始匹配</span>
        </router-link>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useInvoiceStore } from '../stores/invoice'
import { usePaymentStore } from '../stores/payment'
import { useMatchStore } from '../stores/match'
import { invoke } from '@tauri-apps/api/core'

const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()

const ocrOnline = ref(false)

onMounted(async () => {
  try {
    ocrOnline.value = await invoke('ocr_health')
  } catch {
    ocrOnline.value = false
  }
})
</script>
