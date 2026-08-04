<template>
  <div v-if="visible" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white rounded-[10px] shadow-2xl w-[700px] max-h-[85vh] flex flex-col">
      <div class="flex items-center justify-between px-5 py-4 border-b border-gray-100 shrink-0">
        <div class="flex items-center gap-2">
          <h2 class="text-base font-semibold text-gray-800">发票详情</h2>
          <span class="px-2 py-0.5 rounded text-xs font-medium" :class="categoryBadgeClass">{{ categoryLabel }}</span>
          <span v-if="hasIncompleteItineraries" class="inline-flex items-center gap-1 text-orange-500 text-xs" title="有行程字段未完整识别">
            <AppIcon name="alert" :size="13" />
            部分字段需确认
          </span>
        </div>
        <button class="text-gray-400 hover:text-gray-600" aria-label="关闭" @click="$emit('close')">
          <AppIcon name="x" :size="16" />
        </button>
      </div>

      <div class="flex-1 overflow-auto px-5 py-4">
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

        <!-- 行程明细 — 可编辑 -->
        <div v-if="invoice?.itineraries?.length" class="mb-4">
          <h4 class="text-sm font-medium text-gray-700 mb-2">行程明细 ({{ invoice.itineraries.length }})</h4>
          <div class="space-y-2">
            <div v-for="(it, i) in editedItineraries" :key="i"
                 class="rounded p-3 text-sm"
                 :class="it.incomplete_fields?.length ? 'bg-orange-50 border border-orange-200' : 'bg-primary-50'">
              <div class="flex items-center gap-1 mb-2">
                <span class="text-xs font-medium text-gray-500">#{{ i + 1 }}</span>
                <span v-if="it.incomplete_fields?.length"
                      class="inline-flex items-center gap-1 text-xs text-orange-600"
                      title="缺失字段"><AppIcon name="alert" :size="13" />{{ it.incomplete_fields.map(f => fieldLabel(f)).join(', ') }}</span>
              </div>
              <div class="grid grid-cols-2 gap-2">
                <div>
                  <label class="text-xs text-gray-400">时间</label>
                  <input v-model="it.date_time"
                         class="w-full border rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary-100"
                         :class="isFieldIncomplete(it, 'date_time')
                           ? 'border-orange-400 bg-orange-50 focus:border-orange-400'
                           : 'border-gray-300 focus:border-primary-500'" />
                </div>
                <div>
                  <label class="text-xs text-gray-400">金额</label>
                  <input v-model.number="it.amount"
                         type="number" step="0.01"
                         class="w-full border rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary-100"
                         :class="isFieldIncomplete(it, 'amount')
                           ? 'border-orange-400 bg-orange-50 focus:border-orange-400'
                           : 'border-gray-300 focus:border-primary-500'" />
                </div>
                <div>
                  <label class="text-xs text-gray-400">服务商</label>
                  <input v-model="it.provider"
                         class="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100" />
                </div>
                <div>
                  <label class="text-xs text-gray-400">城市</label>
                  <input v-model="it.city"
                         class="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm bg-gray-50 focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100"
                         placeholder="未提取" />
                </div>
                <div>
                  <label class="text-xs text-gray-400">起点</label>
                  <input v-model="it.pickup"
                         class="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100" />
                </div>
                <div>
                  <label class="text-xs text-gray-400">终点</label>
                  <input v-model="it.dropoff"
                         class="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100" />
                </div>
              </div>
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
          <div class="inline-block w-5 h-5 border-2 border-gray-300 border-t-primary-500 rounded-full animate-spin"></div>
        </div>
        <div v-else-if="loadError" class="text-center py-8 text-red-400">
          <p>预览加载失败</p>
        </div>
      </div>

      <div class="px-5 py-3 border-t border-gray-100 flex justify-between items-center gap-2 shrink-0">
        <div class="flex gap-2">
          <AppButton v-if="canOpenFile" @click="handleOpenFile">用系统打开</AppButton>
        </div>
        <div class="flex gap-2">
          <AppButton v-if="hasEdits" variant="primary" @click="handleSaveAndRematch">保存并重新匹配</AppButton>
          <AppButton @click="$emit('close')">关闭</AppButton>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, reactive, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { Invoice, Itinerary } from '../types'
import { getCategoryStyle, getCategoryBadgeClass } from '../utils/category'
import AppButton from './ui/AppButton.vue'
import AppIcon from './ui/AppIcon.vue'

const props = defineProps<{ visible: boolean; invoice: Invoice | null }>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'save', invoice: Invoice): void
}>()

const previewImages = ref<string[]>([])
const loadingPreview = ref(false)
const loadError = ref(false)

// Deep copy itineraries for editing
const editedItineraries = reactive<Itinerary[]>([])

function deepCopy(src: Itinerary[]): Itinerary[] {
  return src.map(it => ({
    date_time: it.date_time,
    provider: it.provider,
    pickup: it.pickup,
    dropoff: it.dropoff,
    amount: it.amount,
    city: it.city,
    incomplete_fields: [...(it.incomplete_fields || [])],
  }))
}

// Reset edited copy when invoice changes
watch(() => props.invoice, (inv) => {
  if (inv?.itineraries) {
    const copy = deepCopy(inv.itineraries)
    editedItineraries.splice(0, editedItineraries.length, ...copy)
  } else {
    editedItineraries.splice(0)
  }
}, { immediate: true })

const hasEdits = computed(() => {
  if (!props.invoice?.itineraries || editedItineraries.length === 0) return false
  const orig = props.invoice.itineraries
  if (orig.length !== editedItineraries.length) return true
  return editedItineraries.some((it, i) =>
    it.date_time !== orig[i].date_time ||
    it.amount !== orig[i].amount ||
    it.provider !== orig[i].provider ||
    it.city !== orig[i].city ||
    it.pickup !== orig[i].pickup ||
    it.dropoff !== orig[i].dropoff
  )
})

const hasIncompleteItineraries = computed(() =>
  editedItineraries.some(it => it.incomplete_fields?.length > 0)
)

function isFieldIncomplete(it: Itinerary, field: string): boolean {
  return it.incomplete_fields?.includes(field) ?? false
}

function fieldLabel(f: string): string {
  const map: Record<string, string> = {
    date_time: '时间', provider: '服务商', pickup: '起点', dropoff: '终点', amount: '金额',
  }
  return map[f] || f
}

function handleSaveAndRematch() {
  if (!props.invoice) return
  const updated: Invoice = {
    ...props.invoice,
    itineraries: editedItineraries.map(it => ({
      ...it,
      // Clear incomplete_fields after manual fix
      incomplete_fields: [],
    })),
  }
  emit('save', updated)
}

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
  if (t === 'Manual') return '手动添加'
  return t
})
const canOpenFile = computed(() => {
  if (!props.invoice) return false
  return props.invoice.source.type === 'Photo' || props.invoice.source.type === 'Pdf'
})

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.visible) emit('close')
}

watch(() => props.visible, (v) => {
  if (v) window.addEventListener('keydown', onKeydown)
  else window.removeEventListener('keydown', onKeydown)
})

onUnmounted(() => window.removeEventListener('keydown', onKeydown))

watch(() => props.visible, async (v) => {
  if (!v || !props.invoice) return
  previewImages.value.forEach(u => URL.revokeObjectURL(u))
  previewImages.value = []
  if (!props.invoice.source.path) {
    loadingPreview.value = false
    return
  }
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
  if (!props.invoice || !props.invoice.source.path) return
  try {
    await invoke('open_file_with_system', { filePath: props.invoice.source.path })
  } catch (e) {
    console.error('打开文件失败:', e)
  }
}
</script>
