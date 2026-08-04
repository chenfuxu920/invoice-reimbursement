<template>
  <div class="space-y-3">
    <div class="flex gap-3">
      <button
        @click="billType = 'wechat'"
        class="px-4 py-2 rounded border transition-colors"
        :class="billType === 'wechat' ? 'bg-emerald-600 text-white border-emerald-600' : 'bg-white text-gray-600 border-gray-300 hover:border-emerald-400'"
      >
        微信账单
      </button>
      <button
        @click="billType = 'alipay'"
        class="px-4 py-2 rounded border transition-colors"
        :class="billType === 'alipay' ? 'bg-primary-600 text-white border-primary-600' : 'bg-white text-gray-600 border-gray-300 hover:border-primary-400'"
      >
        支付宝账单
      </button>
    </div>

    <div
      class="border-2 border-dashed rounded-lg p-6 text-center cursor-pointer
             hover:border-primary-400 hover:bg-primary-50 transition-colors"
      :class="isDragging ? 'border-primary-500 bg-primary-50' : 'border-gray-300'"
      @dragover.prevent="isDragging = true"
      @dragleave="isDragging = false"
      @click="openFilePicker"
    >
      <p class="text-gray-500">拖拽账单文件到此处，或点击选择 Excel 文件</p>
      <p class="text-sm text-gray-400 mt-1">.xlsx / .xls / .csv</p>
    </div>

    <div v-if="loading" class="flex items-center justify-center gap-2 text-primary-600">
      <svg class="animate-spin h-5 w-5" viewBox="0 0 24 24" fill="none">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 0 1 8-8V0C5.4 0 0 5.4 0 12h4z" />
      </svg>
      解析中...
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'

type BillType = 'wechat' | 'alipay'

const billType = ref<BillType>('wechat')
const isDragging = ref(false)
const loading = ref(false)
let unlisten: (() => void) | null = null

const emit = defineEmits<{
  (e: 'import', paths: string[], type: BillType): void
}>()

onMounted(async () => {
  unlisten = await getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === 'drop' && event.payload.paths.length > 0) {
      isDragging.value = false
      emit('import', event.payload.paths, billType.value)
    }
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
})

async function openFilePicker() {
  const selected = await open({
    multiple: false,
    filters: [{
      name: '账单文件',
      extensions: ['xlsx', 'xls', 'csv']
    }]
  })
  if (selected) {
    emit('import', [selected], billType.value)
  }
}
</script>
