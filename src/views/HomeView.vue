<template>
  <div class="space-y-6">
    <!-- OCR 引擎状态卡 -->
    <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-5">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <span class="w-2.5 h-2.5 rounded-full" :class="ocrOnline ? 'bg-emerald-500' : 'bg-red-500'" />
          <span class="font-medium text-gray-800">OCR 识别服务</span>
        </div>
        <AppBadge :tone="ocrOnline ? 'success' : 'danger'">{{ ocrOnline ? '在线' : '离线' }}</AppBadge>
      </div>
      <div v-if="!ocrOnline" class="mt-4 pt-4 border-t border-gray-100 space-y-3">
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0">
            <p class="text-sm font-medium text-gray-700">OCR 模型</p>
            <p class="text-xs text-gray-400">识别扫描件、图片发票（约 20MB）</p>
          </div>
          <AppButton v-if="!downloadingModels" variant="primary" size="sm" @click="downloadModels">下载</AppButton>
          <span v-else class="text-sm text-primary-600">{{ downloadProgress.file }} ({{ downloadProgress.index + 1 }}/{{ downloadProgress.total }})…</span>
        </div>
        <div>
          <button class="text-xs text-gray-400 hover:text-gray-600" @click="showConfig = !showConfig">⚙ 下载地址设置</button>
          <div v-if="showConfig" class="mt-2 flex gap-2">
            <input v-model="modelBaseUrl" class="flex-1 px-2.5 py-1.5 border border-gray-300 rounded-lg text-sm font-mono focus:outline-none focus:border-primary-500"
                   placeholder="https://github.com/.../releases/download/ocr-models-v1" />
            <AppButton size="sm" @click="saveConfig">保存</AppButton>
          </div>
        </div>
      </div>
    </div>

    <!-- 数据统计 -->
    <div class="grid grid-cols-2 lg:grid-cols-4 gap-3">
      <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 text-center">
        <p class="text-3xl font-bold text-primary-600 tabular-nums">{{ invoiceStore.invoices.length }}</p>
        <p class="text-sm text-gray-500 mt-1">已导入发票</p>
      </div>
      <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 text-center">
        <p class="text-3xl font-bold text-emerald-600 tabular-nums">{{ paymentStore.payments.length }}</p>
        <p class="text-sm text-gray-500 mt-1">支付记录</p>
      </div>
      <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 text-center">
        <p class="text-3xl font-bold text-purple-600 tabular-nums">{{ matchStore.matches.length }}</p>
        <p class="text-sm text-gray-500 mt-1">已匹配</p>
      </div>
      <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 text-center">
        <p class="text-3xl font-bold text-gray-600 tabular-nums">{{ matchStore.trips.length }}</p>
        <p class="text-sm text-gray-500 mt-1">已分趟</p>
      </div>
    </div>

    <!-- 流程引导 -->
    <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-5">
      <h3 class="font-medium text-gray-800 mb-3">下一步</h3>
      <p class="text-sm text-gray-500 mb-4">{{ nextStepHint }}</p>
      <div class="flex gap-3 flex-wrap">
        <AppButton v-if="!hasInvoices" variant="primary" @click="$router.push('/import')">导入发票与账单</AppButton>
        <AppButton v-else-if="!hasMatches" variant="primary" @click="$router.push('/match')">开始匹配</AppButton>
        <AppButton v-else variant="primary" @click="$router.push('/export')">前往导出</AppButton>
      </div>
    </div>

    <!-- 快捷操作 -->
    <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-5">
      <h3 class="font-medium text-gray-800 mb-4">快捷操作</h3>
      <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
        <router-link to="/import" class="flex flex-col items-center gap-2 p-4 rounded-lg border border-gray-200 hover:border-primary-300 hover:bg-primary-50 transition-colors">
          <AppIcon name="upload" :size="22" class="text-gray-500" />
          <span class="text-sm font-medium text-gray-700">导入发票</span>
        </router-link>
        <router-link to="/import" class="flex flex-col items-center gap-2 p-4 rounded-lg border border-gray-200 hover:border-emerald-300 hover:bg-emerald-50 transition-colors">
          <AppIcon name="table" :size="22" class="text-gray-500" />
          <span class="text-sm font-medium text-gray-700">导入账单</span>
        </router-link>
        <router-link to="/match" class="flex flex-col items-center gap-2 p-4 rounded-lg border border-gray-200 hover:border-purple-300 hover:bg-purple-50 transition-colors">
          <AppIcon name="link" :size="22" class="text-gray-500" />
          <span class="text-sm font-medium text-gray-700">开始匹配</span>
        </router-link>
        <router-link to="/export" class="flex flex-col items-center gap-2 p-4 rounded-lg border border-gray-200 hover:border-amber-300 hover:bg-amber-50 transition-colors">
          <AppIcon name="download" :size="22" class="text-gray-500" />
          <span class="text-sm font-medium text-gray-700">导出报销表</span>
        </router-link>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useInvoiceStore } from '../stores/invoice'
import { usePaymentStore } from '../stores/payment'
import { useMatchStore } from '../stores/match'
import AppButton from '../components/ui/AppButton.vue'
import AppBadge from '../components/ui/AppBadge.vue'
import AppIcon from '../components/ui/AppIcon.vue'
import { useOcrStatus } from '../composables/ocr'
import { toast } from '../composables/toast'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()

const { ocrOnline } = useOcrStatus()

const downloadingModels = ref(false)
const showConfig = ref(false)
const modelBaseUrl = ref('')
const downloadProgress = ref({ file: '', index: 0, total: 0 })

onMounted(async () => {
  try {
    const config = await invoke<{ model_base_url: string }>('get_ocr_model_config')
    modelBaseUrl.value = config.model_base_url
  } catch { /* 使用默认值 */ }

  await listen<{ file: string; index: number; total: number }>('ocr-download-progress', (e) => {
    downloadProgress.value = e.payload
  })
})

async function downloadModels() {
  downloadingModels.value = true
  try {
    await invoke('download_ocr_models')
    try { ocrOnline.value = await invoke('ocr_health') } catch { /* ignore */ }
    downloadingModels.value = false
    toast(ocrOnline.value
      ? 'OCR 模型下载完成，识别服务已就绪，可直接使用，无需重启。'
      : 'OCR 模型下载完成，但引擎未就绪，请重启应用后使用。', ocrOnline.value ? 'success' : 'error')
  } catch (e) {
    downloadingModels.value = false
    toast(`下载失败: ${e}`, 'error')
  }
}

async function saveConfig() {
  try {
    await invoke('set_ocr_model_config', { modelBaseUrl: modelBaseUrl.value })
    showConfig.value = false
    toast('下载地址已保存', 'success')
  } catch (e) {
    toast(`保存失败: ${e}`, 'error')
  }
}

const hasInvoices = computed(() => invoiceStore.invoices.length > 0)
const hasMatches = computed(() => matchStore.matches.length > 0)
const nextStepHint = computed(() => {
  if (!hasInvoices.value) return '先导入发票与微信/支付宝账单，再进行自动匹配。'
  if (!hasMatches.value) return '发票与账单已就绪，点击开始自动匹配。'
  if (matchStore.trips.length === 0) return '匹配完成，前往导出页确认分趟并生成报销表。'
  return '全部就绪，可随时前往导出页生成报销材料。'
})
</script>
