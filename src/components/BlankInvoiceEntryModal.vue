<template>
  <div v-if="visible" class="fixed inset-0 bg-slate-900/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white/95 rounded-2xl shadow-card-lg animate-scale-in w-[480px] max-h-[85vh] flex flex-col">
      <div class="flex items-center justify-between px-5 py-4 border-b border-slate-100 shrink-0">
        <h2 class="text-base font-semibold text-slate-800">手动添加空发票</h2>
        <button class="text-slate-400 hover:text-slate-600" aria-label="关闭" @click="$emit('close')">
          <AppIcon name="x" :size="16" />
        </button>
      </div>

      <div class="flex-1 overflow-auto px-5 py-4">
        <p class="text-xs text-slate-500 mb-3">
          无需上传电子票据，仅填写类别、时间与金额。匹配支付记录后，对照单该页将留白用于粘贴纸质票据。
        </p>

        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-xs text-slate-500 mb-1">类别 *</label>
            <select v-model="form.category" class="w-full input">
              <option v-for="cat in categoryOptions" :key="cat.value" :value="cat.value">{{ cat.label }}</option>
            </select>
          </div>
          <div>
            <label class="block text-xs text-slate-500 mb-1">日期 *</label>
            <input v-model="form.date" type="date" class="w-full input" />
          </div>
          <div class="col-span-2">
            <label class="block text-xs text-slate-500 mb-1">金额 *</label>
            <input v-model.number="form.amount" type="number" step="0.01" class="w-full input" placeholder="0.00" />
          </div>
          <div>
            <label class="block text-xs text-slate-500 mb-1">销售方</label>
            <input v-model="form.seller_name" class="w-full input" placeholder="选填" />
          </div>
          <div>
            <label class="block text-xs text-slate-500 mb-1">商品/服务</label>
            <input v-model="form.item_name" class="w-full input" placeholder="选填" />
          </div>
          <div class="col-span-2">
            <label class="block text-xs text-slate-500 mb-1">发票号</label>
            <input v-model="form.invoice_number" class="w-full input" placeholder="选填" />
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
import type { Invoice, InvoiceCategory } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import AppButton from './ui/AppButton.vue'
import AppIcon from './ui/AppIcon.vue'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'save', invoice: Invoice): void
}>()

const saveError = ref('')

const isFormValid = computed(() => {
  return form.amount > 0 && !isNaN(form.amount) && form.date.trim() !== ''
})

const categoryOptions = computed(() =>
  (Object.keys(CATEGORY_LABELS) as InvoiceCategory[]).map(v => ({ value: v, label: CATEGORY_LABELS[v] }))
)

const form = reactive({
  invoice_number: '',
  amount: 0,
  seller_name: '',
  item_name: '',
  date: new Date().toISOString().slice(0, 10),
  category: 'Other' as InvoiceCategory,
})

function handleSave() {
  if (!isFormValid.value) {
    saveError.value = '请填写类别、日期与有效金额'
    return
  }
  saveError.value = ''
  const invoice: Invoice = {
    id: 'manual-blank-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8),
    invoice_number: form.invoice_number,
    amount: form.amount,
    seller_name: form.seller_name,
    item_name: form.item_name,
    date: form.date,
    category: form.category,
    source: { type: 'Manual' },
    itineraries: [],
  }
  emit('save', invoice)
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.visible) emit('close')
}

watch(() => props.visible, (v) => {
  if (v) window.addEventListener('keydown', onKeydown)
  else window.removeEventListener('keydown', onKeydown)
}, { immediate: true })

onUnmounted(() => window.removeEventListener('keydown', onKeydown))

watch(() => props.visible, (v) => {
  if (!v) return
  saveError.value = ''
  form.invoice_number = ''
  form.amount = 0
  form.seller_name = ''
  form.item_name = ''
  form.date = new Date().toISOString().slice(0, 10)
  form.category = 'Other'
})
</script>
