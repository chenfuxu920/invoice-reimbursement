<template>
  <div v-if="visible" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white rounded-lg shadow-xl w-[640px] max-h-[85vh] flex flex-col">
      <div class="flex items-center justify-between p-4 border-b shrink-0">
        <div class="flex items-center gap-2">
          <h3 class="font-medium">发票详情</h3>
          <span class="px-2 py-0.5 rounded text-xs font-medium" :class="categoryBadgeClass">{{ categoryLabel }}</span>
        </div>
        <button @click="$emit('close')" class="text-gray-400 hover:text-gray-600 text-xl leading-none">&times;</button>
      </div>

      <div class="p-4 overflow-y-auto flex-1">
        <div class="grid grid-cols-2 gap-3 mb-4">
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">发票编号</p>
            <p class="font-medium">{{ invoice?.invoice_number || '无编号' }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">金额</p>
            <p class="font-medium text-lg">¥{{ invoice?.amount.toFixed(2) }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">销售方</p>
            <p class="font-medium">{{ invoice?.seller_name || '未知' }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">商品/服务</p>
            <p class="font-medium">{{ invoice?.item_name || '未知' }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">开票日期</p>
            <p class="font-medium">{{ invoice?.date || '未知' }}</p>
          </div>
          <div class="bg-gray-50 rounded p-3">
            <p class="text-xs text-gray-500">发票来源</p>
            <p class="font-medium">{{ sourceTypeLabel }}</p>
          </div>
        </div>

        <div v-if="invoice?.itineraries?.length" class="mb-4">
          <h4 class="text-sm font-medium text-gray-700 mb-2">行程明细 ({{ invoice.itineraries.length }})</h4>
          <div class="space-y-2">
            <div v-for="(it, i) in invoice.itineraries" :key="i" class="bg-blue-50 rounded p-2 text-sm">
              <div class="flex justify-between">
                <span class="text-gray-500">{{ it.date_time }}</span>
                <span class="font-medium">¥{{ it.amount.toFixed(2) }}</span>
              </div>
              <p class="text-gray-600">{{ it.provider }} · {{ it.pickup }} → {{ it.dropoff }}</p>
            </div>
          </div>
        </div>

        <div v-if="previewImages.length > 0" class="mb-4">
          <h4 class="text-sm font-medium text-gray-700 mb-2">原始文件预览</h4>
          <div class="space-y-3">
            <div v-for="(img, i) in previewImages" :key="i" class="border rounded overflow-hidden bg-gray-100">
              <img :src="img" class="w-full h-auto" :alt="`第 ${i + 1} 页`" />
              <p v-if="previewImages.length > 1" class="text-xs text-center text-gray-500 py-1 bg-gray-50 border-t">
                第 {{ i + 1 }} / {{ previewImages.length }} 页
              </p>
            </div>
          </div>
        </div>
        <div v-else-if="loadingPreview" class="text-center py-8 text-gray-400">
          <p class="mb-1">正在渲染预览...</p>
          <div class="inline-block w-5 h-5 border-2 border-gray-300 border-t-blue-500 rounded-full animate-spin"></div>
        </div>
        <div v-else-if="loadError" class="text-center py-8 text-red-400">
          <p>预览加载失败</p>
        </div>
      </div>

      <div class="p-4 border-t flex justify-end gap-2 shrink-0">
        <button v-if="canOpenFile" @click="handleOpenFile" class="px-4 py-2 rounded border hover:bg-gray-50 text-sm">
          用系统打开
        </button>
        <button @click="$emit('close')" class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 text-sm">
          关闭
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { Invoice } from '../types'
import { getCategoryStyle, getCategoryBadgeClass } from '../utils/category'

const props = defineProps<{ visible: boolean; invoice: Invoice | null }>()
defineEmits<{ (e: 'close'): void }>()

const previewImages = ref<string[]>([])
const loadingPreview = ref(false)
const loadError = ref(false)

const categoryLabel = computed(() =>
  props.invoice ? getCategoryStyle(props.invoice.category).label : ''
)
const categoryBadgeClass = computed(() =>
  props.invoice ? getCategoryBadgeClass(props.invoice.category) : ''
)
const sourceTypeLabel = computed(() => {
  if (!props.invoice) return ''
  const t = props.invoice.source.type
  if (t === 'Photo') return '拍照/图片'
  if (t === 'Pdf') return 'PDF 文件'
  if (t === 'Link') return '外部链接'
  return t
})
const canOpenFile = computed(() => {
  if (!props.invoice) return false
  return props.invoice.source.type === 'Photo' || props.invoice.source.type === 'Pdf'
})

watch(() => props.visible, async (v) => {
  if (!v || !props.invoice) return
  previewImages.value.forEach(u => URL.revokeObjectURL(u))
  previewImages.value = []
  loadingPreview.value = true
  loadError.value = false
  try {
    const paths: string[] = await invoke('render_pdf_preview', {
      filePath: props.invoice.source.path
    })
    previewImages.value = paths
  } catch (e) {
    console.error('预览加载失败:', e)
    loadError.value = true
  } finally {
    loadingPreview.value = false
  }
})

async function handleOpenFile() {
  if (!props.invoice) return
  try {
    await invoke('open_file_with_system', { filePath: props.invoice.source.path })
  } catch (e) {
    console.error('打开文件失败:', e)
  }
}
</script>
