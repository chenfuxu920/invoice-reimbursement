<template>
  <div v-if="visible" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white rounded-lg shadow-xl w-[900px] max-h-[85vh] flex flex-col">
      <div class="flex items-center justify-between p-4 border-b shrink-0">
        <h3 class="font-medium">手动填写发票信息</h3>
        <button @click="$emit('close')" class="text-gray-400 hover:text-gray-600 text-xl leading-none">&times;</button>
      </div>

      <div class="flex flex-1 overflow-hidden">
        <!-- 左侧缩略图 -->
        <div class="w-[380px] border-r overflow-y-auto bg-gray-50 p-3">
          <p class="text-xs text-gray-500 mb-2">{{ fileName }}</p>
          <div v-if="previewImages.length > 0" class="space-y-3">
            <div v-for="(img, i) in previewImages" :key="i" class="border rounded overflow-hidden bg-white">
              <img :src="img" class="w-full h-auto" :alt="`第 ${i + 1} 页`" />
              <p v-if="previewImages.length > 1" class="text-xs text-center text-gray-500 py-1 bg-gray-50 border-t">
                第 {{ i + 1 }} / {{ previewImages.length }} 页
              </p>
            </div>
          </div>
          <div v-else-if="loadingPreview" class="text-center py-8 text-gray-400">
            <div class="inline-block w-5 h-5 border-2 border-gray-300 border-t-blue-500 rounded-full animate-spin"></div>
            <p class="mt-2 text-sm">正在渲染预览...</p>
          </div>
          <div v-else-if="loadError" class="text-center py-8 text-red-400 text-sm">
            <p>预览加载失败</p>
            <p class="text-xs text-gray-400 mt-1">{{ fileName }}</p>
          </div>
        </div>

        <!-- 右侧表单 -->
        <div class="flex-1 overflow-y-auto p-4">
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs text-gray-500 mb-1">发票号</label>
              <input v-model="form.invoice_number" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="发票号码" />
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">金额 *</label>
              <input v-model.number="form.amount" type="number" step="0.01" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="0.00" />
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">销售方</label>
              <input v-model="form.seller_name" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="销售方名称" />
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">商品/服务</label>
              <input v-model="form.item_name" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="商品或服务名称" />
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">开票日期</label>
              <input v-model="form.date" type="date" class="w-full border rounded px-2 py-1.5 text-sm" />
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">类别</label>
              <select v-model="form.category" class="w-full border rounded px-2 py-1.5 text-sm">
                <option v-for="cat in categoryOptions" :key="cat.value" :value="cat.value">{{ cat.label }}</option>
              </select>
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">来源类型</label>
              <select v-model="form.source.type" class="w-full border rounded px-2 py-1.5 text-sm">
                <option value="Photo">拍照/图片</option>
                <option value="Pdf">PDF 文件</option>
                <option value="Link">外部链接</option>
              </select>
            </div>
          </div>

          <!-- 行程明细录入区（可展开） -->
          <div class="mt-4 border-t pt-3">
            <button @click="showItinerary = !showItinerary"
                    class="flex items-center gap-1 text-sm text-gray-600 hover:text-gray-800">
              <span class="transition-transform" :class="{ 'rotate-90': showItinerary }">▸</span>
              行程明细 ({{ form.itineraries.length }})
            </button>
            <div v-if="showItinerary" class="mt-2 space-y-2">
              <div v-for="(it, i) in form.itineraries" :key="i" class="flex gap-2 items-start bg-gray-50 rounded p-2">
                <input v-model="it.date_time" class="flex-1 border rounded px-2 py-1 text-xs" placeholder="时间" />
                <input v-model="it.provider" class="flex-1 border rounded px-2 py-1 text-xs" placeholder="平台" />
                <input v-model="it.pickup" class="flex-1 border rounded px-2 py-1 text-xs" placeholder="起点" />
                <input v-model="it.dropoff" class="flex-1 border rounded px-2 py-1 text-xs" placeholder="终点" />
                <input v-model.number="it.amount" type="number" step="0.01" class="w-20 border rounded px-2 py-1 text-xs" placeholder="金额" />
                <button @click="form.itineraries.splice(i, 1)" class="text-gray-400 hover:text-red-500 text-sm">✕</button>
              </div>
              <button @click="addItinerary" class="text-xs text-blue-600 hover:text-blue-800">+ 添加行程</button>
            </div>
          </div>
        </div>
      </div>

      <div class="p-4 border-t flex justify-end gap-2 shrink-0">
        <button @click="$emit('close')" class="px-4 py-2 rounded border hover:bg-gray-50 text-sm">取消</button>
        <button @click="handleSave" :disabled="!form.amount" class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-50 text-sm">保存</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, watch, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { Invoice, InvoiceCategory, InvoiceSource, Itinerary } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'

const props = defineProps<{ visible: boolean; filePath: string; errorId: string }>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'save', invoice: Invoice, errorId: string): void
}>()

const previewImages = ref<string[]>([])
const loadingPreview = ref(false)
const loadError = ref(false)
const showItinerary = ref(false)

const categoryOptions = computed(() =>
  (Object.keys(CATEGORY_LABELS) as InvoiceCategory[]).map(v => ({ value: v, label: CATEGORY_LABELS[v] }))
)

const fileName = computed(() => {
  const parts = props.filePath.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || props.filePath
})

const form = reactive({
  invoice_number: '',
  amount: 0,
  seller_name: '',
  item_name: '',
  date: new Date().toISOString().slice(0, 10),
  category: 'Other' as InvoiceCategory,
  source: { type: 'Pdf' as InvoiceSource['type'], path: props.filePath },
  itineraries: [] as Itinerary[],
})

function addItinerary() {
  form.itineraries.push({ date_time: '', provider: '', pickup: '', dropoff: '', amount: 0 })
}

function handleSave() {
  const invoice: Invoice = {
    id: 'manual-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8),
    invoice_number: form.invoice_number,
    amount: form.amount,
    seller_name: form.seller_name,
    item_name: form.item_name,
    date: form.date,
    category: form.category,
    source: { type: form.source.type, path: props.filePath },
    itineraries: form.itineraries.filter(it => it.date_time || it.provider || it.pickup || it.dropoff || it.amount),
  }
  emit('save', invoice, props.errorId)
}

watch(() => props.visible, async (v) => {
  if (!v || !props.filePath) return
  previewImages.value.forEach(u => URL.revokeObjectURL(u))
  previewImages.value = []
  loadingPreview.value = true
  loadError.value = false
  form.source.path = props.filePath
  try {
    const paths: string[] = await invoke('render_pdf_preview', { filePath: props.filePath })
    previewImages.value = paths
  } catch (e) {
    console.error('预览加载失败:', e)
    loadError.value = true
  } finally {
    loadingPreview.value = false
  }
})
</script>
