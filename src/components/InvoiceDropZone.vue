<template>
  <div>
    <div
      class="relative overflow-hidden rounded-2xl border-2 border-dashed transition-all duration-300 cursor-pointer group"
      :class="zoneClass"
      @dragover.prevent="isDragging = true"
      @dragleave="isDragging = false"
      @click="openFilePicker"
    >
      <!-- 拖入高亮光晕 -->
      <div v-if="isDragging" class="absolute inset-0 pointer-events-none">
        <div class="absolute inset-0 bg-gradient-to-br from-primary-500/15 to-accent-500/15 animate-fade-in" />
        <div class="absolute inset-0 rounded-2xl ring-2 ring-primary-500/60 ring-offset-2 ring-offset-white" />
      </div>

      <div class="py-12 px-8 flex flex-col items-center text-center">
        <div v-if="loading" class="flex flex-col items-center gap-3">
          <span class="w-14 h-14 rounded-2xl bg-primary-50 text-primary-600 flex items-center justify-center animate-pulse-soft">
            <Loader2 :size="26" class="animate-spin" />
          </span>
          <p class="text-sm font-medium text-primary-700">正在识别...</p>
        </div>
        <template v-else>
          <span class="w-16 h-16 rounded-2xl bg-gradient-to-br from-primary-500 to-accent-500 text-white shadow-glow flex items-center justify-center mb-4 transition-transform duration-300 group-hover:scale-110 group-hover:-rotate-3"
                :class="isDragging ? 'scale-110 -rotate-3' : ''">
            <UploadCloud :size="30" />
          </span>
          <p class="font-display text-lg font-bold text-slate-800">
            {{ isDragging ? '松开，自动分类识别' : '把发票和账单拖到这里' }}
          </p>
          <p class="text-sm text-slate-400 mt-1.5">
            也可点击选择文件 · 自动识别：PDF / 图片 → 发票，Excel → 账单（微信/支付宝自动区分）
          </p>
        </template>
      </div>
    </div>

    <!-- 支持格式提示条 -->
    <div class="mt-3 flex flex-wrap items-center gap-2">
      <span class="chip bg-white text-slate-500 border border-slate-200 shadow-card"><FileText :size="12" /> PDF 发票</span>
      <span class="chip bg-white text-slate-500 border border-slate-200 shadow-card"><Image :size="12" /> JPG / PNG 发票</span>
      <span class="chip bg-white text-slate-500 border border-slate-200 shadow-card"><Table2 :size="12" /> Excel 账单</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { UploadCloud, FileText, Table2, Image } from 'lucide-vue-next'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'

defineProps<{ loading?: boolean }>()
const emit = defineEmits<{
  (e: 'files-selected', paths: string[]): void
  (e: 'bills-import', paths: string[]): void
}>()
const isDragging = ref(false)
let unlisten: (() => void) | null = null

const INVOICE_EXT = ['pdf', 'jpg', 'jpeg', 'png']
const BILL_EXT = ['xlsx', 'xls', 'csv']

function classify(paths: string[]) {
  const invoices: string[] = []
  const bills: string[] = []
  for (const p of paths) {
    const ext = p.toLowerCase().split('.').pop() || ''
    if (BILL_EXT.includes(ext)) bills.push(p)
    else invoices.push(p)
  }
  return { invoices, bills }
}

onMounted(async () => {
  unlisten = await getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === 'drop' && event.payload.paths.length > 0) {
      isDragging.value = false
      const { invoices, bills } = classify(event.payload.paths)
      if (invoices.length) emit('files-selected', invoices)
      if (bills.length) emit('bills-import', bills)
    }
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
})

async function openFilePicker() {
  const selected = await open({
    multiple: true,
    filters: [{
      name: '发票与账单',
      extensions: [...INVOICE_EXT, ...BILL_EXT],
    }],
  })
  if (!selected) return
  const { invoices, bills } = classify(Array.isArray(selected) ? selected : [selected])
  if (invoices.length) emit('files-selected', invoices)
  if (bills.length) emit('bills-import', bills)
}

const zoneClass = isDragging
  ? 'border-primary-500 bg-primary-50/60 shadow-glow'
  : 'border-slate-300 bg-white/80 hover:border-primary-400 hover:bg-primary-50/30 hover:shadow-card'
</script>
