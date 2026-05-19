<template>
  <div v-if="visible" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white rounded-lg shadow-xl w-[520px] max-h-[80vh] overflow-auto">
      <div class="p-4 border-b">
        <h3 class="font-medium">调整匹配</h3>
      </div>
      <div class="p-4">
        <p class="text-sm text-gray-500 mb-3">为发票选择对应的支付记录：</p>
        <div class="bg-blue-50 rounded p-3 mb-3">
          <p class="font-medium">{{ invoice?.invoice_number || '无编号' }}</p>
          <p class="text-sm">¥{{ invoice?.amount.toFixed(2) }} - {{ invoice?.seller_name }}</p>
        </div>
        <div class="flex gap-2 mb-2">
          <input v-model="searchText" type="text" placeholder="搜索商户名、交易号..."
                 class="flex-1 px-3 py-1.5 text-sm border rounded focus:outline-none focus:ring-1 focus:ring-blue-400" />
          <select v-model="sourceFilter"
                  class="px-2 py-1.5 text-sm border rounded focus:outline-none focus:ring-1 focus:ring-blue-400">
            <option value="all">全部</option>
            <option value="Wechat">微信</option>
            <option value="Alipay">支付宝</option>
          </select>
        </div>
        <div class="flex items-center gap-2 mb-3 text-sm">
          <span class="text-gray-500 shrink-0">时间</span>
          <input v-model="timeStart" type="date"
                 class="flex-1 px-2 py-1 text-sm border rounded focus:outline-none focus:ring-1 focus:ring-blue-400" />
          <span class="text-gray-400">~</span>
          <input v-model="timeEnd" type="date"
                 class="flex-1 px-2 py-1 text-sm border rounded focus:outline-none focus:ring-1 focus:ring-blue-400" />
          <button v-if="timeStart || timeEnd" @click="timeStart = ''; timeEnd = ''"
                  class="text-xs text-blue-500 hover:text-blue-700 shrink-0">清除</button>
        </div>
        <div class="space-y-2 max-h-[40vh] overflow-auto">
          <label v-for="p in filteredPayments" :key="p.id"
                 class="flex items-center gap-2 p-2 rounded border cursor-pointer hover:bg-gray-50"
                 :class="selectedIds.has(p.id) ? 'border-blue-500 bg-blue-50' : 'border-gray-200'">
            <input type="checkbox" :checked="selectedIds.has(p.id)" @change="togglePayment(p.id)" />
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <p class="text-sm font-medium truncate">{{ p.merchant_name }}</p>
                <span class="shrink-0 text-xs px-1.5 py-0.5 rounded"
                      :class="p.source === 'Wechat' ? 'bg-green-100 text-green-700' : 'bg-blue-100 text-blue-700'">
                  {{ p.source === 'Wechat' ? '微信' : '支付宝' }}
                </span>
              </div>
              <div class="flex items-center gap-2 text-xs mt-0.5">
                <span class="text-gray-700 font-medium">¥{{ p.amount.toFixed(2) }}</span>
                <template v-if="p.refund_amount > 0 || p.discount > 0">
                  <span class="text-gray-300">|</span>
                  <span v-if="p.refund_amount > 0" class="text-red-400">退款 ¥{{ p.refund_amount.toFixed(2) }}</span>
                  <span v-if="p.refund_amount > 0 && p.discount > 0" class="text-gray-300"> </span>
                  <span v-if="p.discount > 0" class="text-green-400">优惠 ¥{{ p.discount.toFixed(2) }}</span>
                </template>
                <span class="text-gray-300">|</span>
                <span class="text-gray-400">{{ formatTime(p.transaction_time) }}</span>
              </div>
            </div>
            <span v-if="currentPaymentIds.has(p.id)" class="shrink-0 text-xs text-blue-500">当前匹配</span>
          </label>
          <div v-if="filteredPayments.length === 0" class="text-center py-4 text-sm text-gray-400">无匹配结果</div>
        </div>
        <p class="text-xs text-gray-400 mt-2">已选 {{ selectedIds.size }} 条，共 {{ filteredPayments.length }} 条可见</p>
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
import { ref, computed, watch } from 'vue'
import type { Invoice, PaymentRecord } from '../types'

const props = defineProps<{
  visible: boolean
  invoice: Invoice | null
  currentPayments: PaymentRecord[]
  availablePayments: PaymentRecord[]
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'confirm', invoice: Invoice, paymentIds: string[]): void
}>()

const selectedIds = ref<Set<string>>(new Set())
const searchText = ref('')
const sourceFilter = ref<'all' | 'Wechat' | 'Alipay'>('all')
const timeStart = ref('')
const timeEnd = ref('')

const currentPaymentIds = computed(() => new Set(props.currentPayments.map(p => p.id)))

const allPayments = computed(() => {
  return [...props.currentPayments, ...props.availablePayments]
    .sort((a, b) => b.transaction_time.localeCompare(a.transaction_time))
})

const filteredPayments = computed(() => {
  return allPayments.value.filter(p => {
    if (sourceFilter.value !== 'all' && p.source !== sourceFilter.value) return false
    if (timeStart.value || timeEnd.value) {
      const d = p.transaction_time.slice(0, 10)
      if (timeStart.value && d < timeStart.value) return false
      if (timeEnd.value && d > timeEnd.value) return false
    }
    if (searchText.value) {
      const q = searchText.value.toLowerCase()
      return p.merchant_name.toLowerCase().includes(q)
        || p.transaction_id.toLowerCase().includes(q)
    }
    return true
  })
})

watch(() => props.visible, (v) => {
  if (v) {
    selectedIds.value = new Set(props.currentPayments.map(p => p.id))
    searchText.value = ''
    sourceFilter.value = 'all'
    timeStart.value = ''
    timeEnd.value = ''
  }
})

function formatTime(t: string) {
  if (t.length >= 16) return t.slice(5, 16)
  if (t.length >= 10) return t.slice(5, 10)
  return t
}

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
