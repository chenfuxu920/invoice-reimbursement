<template>
  <div>
    <!-- 选择按钮 -->
    <button
      class="w-full group border-2 border-dashed rounded-2xl py-6 px-6 text-center cursor-pointer transition-all duration-300 flex flex-col items-center gap-2"
      :class="isDragging ? 'border-primary-500 bg-primary-50/60 shadow-glow' : 'border-slate-300 bg-white/60 hover:border-primary-400 hover:bg-primary-50/30 hover:shadow-card'"
      @dragover.prevent="isDragging = true"
      @dragleave="isDragging = false"
      @click="openFilePicker"
    >
      <span class="w-12 h-12 rounded-xl bg-gradient-to-br from-emerald-500 to-teal-500 text-white shadow-glow-sm flex items-center justify-center transition-transform duration-300 group-hover:scale-110">
        <FileSpreadsheet :size="22" />
      </span>
      <template v-if="!loading">
        <p class="text-sm font-medium text-slate-700">选择账单文件（微信 / 支付宝自动识别）</p>
        <p class="text-xs text-slate-400">.xlsx / .xls / .csv（拖拽请使用上方大拖拽区）</p>
      </template>
      <p v-else class="flex items-center gap-2 text-sm text-primary-700">
        <Loader2 :size="16" class="animate-spin" /> 解析中...
      </p>
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { FileSpreadsheet, Loader2 } from 'lucide-vue-next'
import { open } from '@tauri-apps/plugin-dialog'

const isDragging = ref(false)
const loading = ref(false)

const emit = defineEmits<{
  (e: 'import', paths: string[]): void
}>()

// ponytail: 拖拽统一由 InvoiceDropZone 大区处理（webview 拖拽事件会广播给所有监听者，
// 若此处也监听会导致账单被重复导入），此处仅保留点击选择。

async function openFilePicker() {
  const selected = await open({
    multiple: false,
    filters: [{ name: '账单文件', extensions: ['xlsx', 'xls', 'csv'] }],
  })
  if (selected) {
    emit('import', [selected])
  }
}
</script>
