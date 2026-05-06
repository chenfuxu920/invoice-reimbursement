<template>
  <div v-if="visible" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white rounded-lg shadow-xl w-[480px] max-h-[80vh] overflow-auto">
      <div class="p-4 border-b">
        <h3 class="font-medium">调整匹配</h3>
      </div>
      <div class="p-4">
        <p class="text-sm text-gray-500 mb-3">为发票选择对应的支付记录：</p>
        <div class="bg-blue-50 rounded p-3 mb-3">
          <p class="font-medium">{{ invoice?.invoice_number || '无编号' }}</p>
          <p class="text-sm">¥{{ invoice?.amount.toFixed(2) }} - {{ invoice?.seller_name }}</p>
        </div>
        <div class="space-y-2 max-h-[40vh] overflow-auto">
          <label v-for="p in availablePayments" :key="p.id"
                 class="flex items-center gap-2 p-2 rounded border cursor-pointer hover:bg-gray-50"
                 :class="selectedIds.has(p.id) ? 'border-blue-500 bg-blue-50' : 'border-gray-200'">
            <input type="checkbox" :checked="selectedIds.has(p.id)" @change="togglePayment(p.id)" />
            <div class="flex-1">
              <p class="text-sm font-medium">{{ p.merchant_name }}</p>
              <p class="text-xs text-gray-500">{{ p.transaction_time }} · ¥{{ p.amount.toFixed(2) }}</p>
            </div>
          </label>
        </div>
      </div>
      <div class="p-4 border-t flex justify-end gap-2">
        <button @click="$emit('close')" class="px-4 py-2 rounded border hover:bg-gray-50">取消</button>
        <button @click="confirmMatch" :disabled="selectedIds.size === 0"
                class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-50">
          确认匹配
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import type { Invoice, PaymentRecord } from '../types'

const props = defineProps<{
  visible: boolean
  invoice: Invoice | null
  availablePayments: PaymentRecord[]
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'confirm', invoice: Invoice, paymentIds: string[]): void
}>()

const selectedIds = ref<Set<string>>(new Set())

watch(() => props.visible, (v) => {
  if (v) selectedIds.value = new Set()
})

function togglePayment(id: string) {
  const next = new Set(selectedIds.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  selectedIds.value = next
}

function confirmMatch() {
  if (props.invoice && selectedIds.value.size > 0) {
    emit('confirm', props.invoice, Array.from(selectedIds.value))
  }
}
</script>
