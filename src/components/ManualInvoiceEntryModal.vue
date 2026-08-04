<template>
  <div v-if="visible" class="fixed inset-0 bg-slate-900/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white/95 rounded-2xl shadow-card-lg animate-scale-in w-[900px] max-h-[85vh] flex flex-col">
      <div class="flex items-center justify-between px-5 py-4 border-b border-slate-100 shrink-0">
        <h2 class="text-base font-semibold text-slate-800">手动填写发票信息</h2>
        <button class="text-slate-400 hover:text-slate-600" aria-label="关闭" @click="$emit('close')">
          <AppIcon name="x" :size="16" />
        </button>
      </div>

      <div class="flex flex-1 overflow-hidden">
        <!-- 左侧缩略图 -->
        <div class="w-[380px] border-r overflow-y-auto bg-slate-50 p-4">
          <p class="text-xs text-slate-500 mb-2">{{ fileName }}</p>
          <div v-if="previewImages.length > 0" class="space-y-3">
            <div v-for="(img, i) in previewImages" :key="i" class="border rounded overflow-hidden bg-white">
              <img :src="img" class="w-full h-auto" :alt="`第 ${i + 1} 页`" />
              <p v-if="previewImages.length > 1" class="text-xs text-center text-slate-500 py-1 bg-slate-50 border-t">
                第 {{ i + 1 }} / {{ previewImages.length }} 页
              </p>
            </div>
          </div>
          <div v-else-if="loadingPreview" class="text-center py-8 text-slate-400">
            <div class="inline-block w-5 h-5 border-2 border-gray-300 border-t-primary-500 rounded-full animate-spin"></div>
            <p class="mt-2 text-sm">正在渲染预览...</p>
          </div>
          <div v-else-if="loadError" class="text-center py-8 text-red-400 text-sm">
            <p>预览加载失败</p>
            <p class="text-xs text-slate-400 mt-1">{{ fileName }}</p>
          </div>
        </div>

        <!-- 右侧表单 -->
        <div class="flex-1 overflow-y-auto px-5 py-4">
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs text-slate-500 mb-1">发票号</label>
              <input v-model="form.invoice_number" class="w-full input" placeholder="发票号码" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">金额 *</label>
              <input v-model.number="form.amount" type="number" step="0.01" class="w-full input" placeholder="0.00" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">销售方</label>
              <input v-model="form.seller_name" class="w-full input" placeholder="销售方名称" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">商品/服务</label>
              <input v-model="form.item_name" class="w-full input" placeholder="商品或服务名称" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">开票日期</label>
              <input v-model="form.date" type="date" class="w-full input" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">类别</label>
              <select v-model="form.category" class="w-full input">
                <option v-for="cat in categoryOptions" :key="cat.value" :value="cat.value">{{ cat.label }}</option>
              </select>
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">来源类型</label>
              <select v-model="form.source.type" class="w-full input">
                <option value="Photo">拍照/图片</option>
                <option value="Pdf">PDF 文件</option>
                <option value="Link">外部链接</option>
              </select>
            </div>
          </div>

          <!-- 行程明细录入区（可展开） -->
          <div class="mt-4 border-t pt-3">
            <button @click="showItinerary = !showItinerary"
                    class="flex items-center gap-1 text-sm text-slate-600 hover:text-slate-800">
              <AppIcon name="chevron-down" :size="14" class="transition-transform" :class="showItinerary ? 'rotate-0' : '-rotate-90'" />
              行程明细 ({{ form.itineraries.length }})
            </button>
            <div v-if="showItinerary" class="mt-2 space-y-2">
              <div v-for="(it, i) in form.itineraries" :key="i" class="flex gap-2 items-start bg-slate-50 rounded p-2">
                <input v-model="it.date_time" class="flex-1 input-sm" placeholder="时间" />
                <input v-model="it.provider" class="flex-1 input-sm" placeholder="平台" />
                <input v-model="it.pickup" class="flex-1 input-sm" placeholder="起点" />
                <input v-model="it.dropoff" class="flex-1 input-sm" placeholder="终点" />
                <input v-model.number="it.amount" type="number" step="0.01" class="w-20 input-sm" placeholder="金额" />
                <button @click="form.itineraries.splice(i, 1)" class="text-slate-400 hover:text-red-500 mt-1" aria-label="删除行程">
                  <AppIcon name="x" :size="14" />
                </button>
              </div>
              <button @click="addItinerary" class="text-xs text-primary-600 hover:text-primary-700">+ 添加行程</button>
            </div>
          </div>
        </div>
      </div>

      <div class="px-5 py-3 border-t border-slate-100 flex justify-end gap-2 items-center shrink-0">
        <p v-if="saveError" class="flex-1 text-sm text-red-500 self-center">{{ saveError }}</p>
        <AppButton @click="$emit('close')">取消</AppButton>
        <AppButton variant="primary" :disabled="!isFormValid" @click="handleSave">保存</AppButton>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, watch, computed, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { Invoice, InvoiceCategory, InvoiceSource, Itinerary } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import AppButton from './ui/AppButton.vue'
import AppIcon from './ui/AppIcon.vue'

const props = defineProps<{ visible: boolean; filePath: string; errorId: string }>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'save', invoice: Invoice, errorId: string): void
}>()

const previewImages = ref<string[]>([])
const loadingPreview = ref(false)
const loadError = ref(false)
const showItinerary = ref(false)
const saveError = ref('')

const isFormValid = computed(() => {
  return form.amount > 0 && !isNaN(form.amount) && form.invoice_number.trim() !== ''
})

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
  form.itineraries.push({ date_time: '', provider: '', pickup: '', dropoff: '', amount: 0, incomplete_fields: [] })
}

function handleSave() {
  if (!isFormValid.value) {
    saveError.value = '请填写发票号与有效金额'
    return
  }
  saveError.value = ''
  const invoice: Invoice = {
    id: 'manual-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8),
    invoice_number: form.invoice_number,
    amount: form.amount,
    seller_name: form.seller_name,
    item_name: form.item_name,
    date: form.date,
    category: form.category,
    source: { type: form.source.type, path: props.filePath },
    itineraries: form.itineraries.filter(it =>
      it.date_time || it.provider || it.pickup || it.dropoff || (!isNaN(it.amount) && it.amount > 0)
    ),
  }
  emit('save', invoice, props.errorId)
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.visible) emit('close')
}

watch(() => props.visible, (v) => {
  if (v) window.addEventListener('keydown', onKeydown)
  else window.removeEventListener('keydown', onKeydown)
}, { immediate: true })

onUnmounted(() => window.removeEventListener('keydown', onKeydown))

watch(() => props.visible, async (v) => {
  if (!v || !props.filePath) return
  // data URLs，无需手动清理
  previewImages.value = []
  loadingPreview.value = true
  loadError.value = false
  saveError.value = ''
  form.source.path = props.filePath
  form.invoice_number = ''
  form.amount = 0
  form.seller_name = ''
  form.item_name = ''
  form.date = new Date().toISOString().slice(0, 10)
  form.category = 'Other'
  form.itineraries = []
  showItinerary.value = false
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
