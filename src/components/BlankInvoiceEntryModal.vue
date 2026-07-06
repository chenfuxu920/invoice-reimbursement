<template>
  <div v-if="visible" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white rounded-lg shadow-xl w-[480px] flex flex-col">
      <div class="flex items-center justify-between p-4 border-b shrink-0">
        <h3 class="font-medium">手动添加空发票</h3>
        <button @click="$emit('close')" class="text-gray-400 hover:text-gray-600 text-xl leading-none">&times;</button>
      </div>

      <div class="p-4 overflow-y-auto">
        <p class="text-xs text-gray-500 mb-3">
          无需上传电子票据，仅填写类别、时间与金额。匹配支付记录后，对照单该页将留白用于粘贴纸质票据。
        </p>

        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-xs text-gray-500 mb-1">类别 *</label>
            <select v-model="form.category" class="w-full border rounded px-2 py-1.5 text-sm">
              <option v-for="cat in categoryOptions" :key="cat.value" :value="cat.value">{{ cat.label }}</option>
            </select>
          </div>
          <div>
            <label class="block text-xs text-gray-500 mb-1">日期 *</label>
            <input v-model="form.date" type="date" class="w-full border rounded px-2 py-1.5 text-sm" />
          </div>
          <div class="col-span-2">
            <label class="block text-xs text-gray-500 mb-1">金额 *</label>
            <input v-model.number="form.amount" type="number" step="0.01" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="0.00" />
          </div>
          <div>
            <label class="block text-xs text-gray-500 mb-1">销售方</label>
            <input v-model="form.seller_name" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="选填" />
          </div>
          <div>
            <label class="block text-xs text-gray-500 mb-1">商品/服务</label>
            <input v-model="form.item_name" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="选填" />
          </div>
          <div class="col-span-2">
            <label class="block text-xs text-gray-500 mb-1">发票号</label>
            <input v-model="form.invoice_number" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="选填" />
          </div>
        </div>
      </div>

      <div class="p-4 border-t flex justify-end gap-2 shrink-0">
        <p v-if="saveError" class="flex-1 text-sm text-red-500 self-center">{{ saveError }}</p>
        <button @click="$emit('close')" class="px-4 py-2 rounded border hover:bg-gray-50 text-sm">取消</button>
        <button @click="handleSave" :disabled="!isFormValid" class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-50 text-sm">保存</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, watch, computed } from 'vue'
import type { Invoice, InvoiceCategory } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'

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
