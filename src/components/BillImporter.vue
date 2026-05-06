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
      @drop.prevent="handleDrop"
      @click="openFilePicker"
    >
      <p class="text-gray-500">拖拽账单文件到此处，或点击选择 Excel 文件</p>
      <p class="text-sm text-gray-400 mt-1">.xlsx / .xls / .csv</p>
      <input ref="fileInput" type="file" class="hidden" accept=".xlsx,.xls,.csv" @change="handleFileSelect" />
    </div>

    <div v-if="loading" class="text-center text-blue-500">
      <span class="animate-spin inline-block mr-2">⏳</span> 解析中...
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'

type BillType = 'wechat' | 'alipay'

const billType = ref<BillType>('wechat')
const isDragging = ref(false)
const loading = ref(false)
const fileInput = ref<HTMLInputElement>()

const emit = defineEmits<{
  (e: 'import', filePath: string, type: BillType): void
}>()

function openFilePicker() {
  fileInput.value?.click()
}

function handleDrop(e: DragEvent) {
  isDragging.value = false
  const files = e.dataTransfer?.files
  if (files && files.length > 0) {
    emit('import', files[0].name, billType.value)
  }
}

function handleFileSelect(e: Event) {
  const input = e.target as HTMLInputElement
  if (input.files && input.files.length > 0) {
    emit('import', input.files[0].name, billType.value)
  }
}
</script>
