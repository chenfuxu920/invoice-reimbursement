<template>
  <div class="max-w-4xl mx-auto">
    <h2 class="text-2xl font-bold mb-6">欢迎使用发票报销助手 v0.4.0</h2>

    <!-- 引擎状态 -->
    <div class="bg-white rounded-lg border p-4 shadow-sm mb-6">
      <div class="flex items-center justify-between mb-2">
        <div class="flex items-center gap-3">
          <span class="w-3 h-3 rounded-full" :class="ocrOnline ? 'bg-green-500' : 'bg-red-500'"></span>
          <span class="font-medium">OCR 识别服务</span>
        </div>
        <span class="text-sm" :class="ocrOnline ? 'text-green-600' : 'text-red-500'">
          {{ ocrOnline ? '在线' : '离线' }}
        </span>
      </div>

      <!-- OCR 模型下载（离线时显示） -->
      <div v-if="!ocrOnline" class="mt-3 pt-3 border-t border-gray-100 space-y-3">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2 min-w-0">
            <span class="text-sm font-medium shrink-0">OCR 模型</span>
            <span class="text-xs text-gray-400 truncate">识别扫描件、图片发票（约 20MB）</span>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <button v-if="!downloadingModels" @click="downloadModels"
              class="text-sm px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors">下载</button>
            <span v-if="downloadingModels" class="text-sm text-blue-600">
              {{ downloadProgress.file }} ({{ downloadProgress.index + 1 }}/{{ downloadProgress.total }})…
            </span>
          </div>
        </div>

        <!-- 下载地址设置 -->
        <div class="pt-1">
          <button @click="showConfig = !showConfig"
            class="text-xs text-gray-400 hover:text-gray-600">⚙ 下载地址设置</button>
          <div v-if="showConfig" class="mt-2">
            <div class="flex gap-2">
              <input v-model="modelBaseUrl"
                class="flex-1 px-2 py-1 border rounded text-sm font-mono"
                placeholder="https://github.com/.../releases/download/ocr-models-v1">
              <button @click="saveConfig"
                class="px-3 py-1 bg-gray-100 rounded hover:bg-gray-200 text-sm">保存</button>
            </div>
            <p class="text-xs text-gray-400 mt-1">默认使用 GitHub Releases，可改为自建镜像加速下载</p>
          </div>
        </div>
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
import { listen } from '@tauri-apps/api/event'

const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()

const ocrOnline = ref(false)
const downloadingModels = ref(false)
const showConfig = ref(false)
const modelBaseUrl = ref('')
const downloadProgress = ref({ file: '', index: 0, total: 0 })

onMounted(async () => {
  try { ocrOnline.value = await invoke('ocr_health') } catch { ocrOnline.value = false }
  try {
    const config = await invoke<{ model_base_url: string }>('get_ocr_model_config')
    modelBaseUrl.value = config.model_base_url
  } catch { /* 使用默认值 */ }

  await listen<{ file: string; index: number; total: number }>('ocr-download-progress', (e) => {
    downloadProgress.value = e.payload
  })
  await listen('ocr-download-complete', async () => {
    downloadingModels.value = false
    try { ocrOnline.value = await invoke('ocr_health') } catch { /* ignore */ }
  })
})

async function downloadModels() {
  downloadingModels.value = true
  try {
    await invoke('download_ocr_models')
  } catch (e) {
    downloadingModels.value = false
    alert(`下载失败: ${e}`)
  }
}

async function saveConfig() {
  try {
    await invoke('set_ocr_model_config', { modelBaseUrl: modelBaseUrl.value })
    showConfig.value = false
  } catch (e) {
    alert(`保存失败: ${e}`)
  }
}
</script>
