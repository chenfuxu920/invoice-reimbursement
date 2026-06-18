<template>
  <div class="space-y-3">
    <div class="flex gap-3">
      <button
        @click="billType = 'wechat'"
        class="px-4 py-2 rounded border transition-colors"
        :class="billType === 'wechat' ? 'bg-green-500 text-white border-green-500' : 'bg-white text-gray-600 border-gray-300 hover:border-green-300'"
      >
        微信账单
      </button>
      <button
        @click="billType = 'alipay'"
        class="px-4 py-2 rounded border transition-colors"
        :class="billType === 'alipay' ? 'bg-blue-500 text-white border-blue-500' : 'bg-white text-gray-600 border-gray-300 hover:border-blue-300'"
      >
        支付宝账单
      </button>
    </div>

    <div
      class="border-2 border-dashed rounded-lg p-6 text-center cursor-pointer
             hover:border-blue-400 hover:bg-blue-50 transition-colors"
      :class="isDragging ? 'border-blue-500 bg-blue-50' : 'border-gray-300'"
      @dragover.prevent="isDragging = true"
      @dragleave="isDragging = false"
      @click="openFilePicker"
    >
      <p class="text-gray-500">拖拽账单文件到此处，或点击选择 Excel 文件</p>
      <p class="text-sm text-gray-400 mt-1">.xlsx / .xls / .csv</p>
    </div>

    <div v-if="loading" class="text-center text-blue-500">
      <span class="animate-spin inline-block mr-2">⏳</span> 解析中...
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
