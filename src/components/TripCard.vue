<template>
  <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-5 space-y-4">
    <div class="flex items-center justify-between flex-wrap gap-2">
      <div class="flex items-center gap-3 flex-wrap">
        <AppBadge tone="info">出差 {{ index }}</AppBadge>
        <span class="font-medium">目的地：{{ trip.destination || '未设置' }}</span>
        <span class="text-sm text-gray-600">{{ trip.travelStart }} 至 {{ trip.travelEnd }}</span>
      </div>
      <div class="text-sm text-gray-500">
        城市间交通 {{ trip.ticketIds.length }} · 发票 {{ trip.matches.length }} · 合计
        <span class="font-medium text-gray-800">¥{{ tripTotal.toFixed(2) }}</span>
      </div>
    </div>

    <ReimbursementForm :model-value="formModel" @update="handleFormUpdate" />

    <div class="border border-gray-200 rounded-lg">
      <button @click="showInvoices = !showInvoices" :aria-expanded="showInvoices"
              class="w-full flex items-center justify-between px-3 py-2 text-sm text-gray-600 hover:bg-gray-50">
        <span>发票明细（{{ trip.matches.length }}）</span>
        <span>{{ showInvoices ? '▾' : '▸' }}</span>
      </button>
      <div v-if="showInvoices" class="divide-y divide-gray-100">
        <div v-for="m in trip.matches" :key="m.invoice_id"
             class="flex items-center gap-2 px-3 py-2 text-sm hover:bg-gray-50 cursor-pointer"
             @click="openDetail(m.invoice)">
          <span class="w-20 shrink-0 text-xs font-medium" :class="getCategoryBadgeClass(m.invoice.category)">
            {{ CATEGORY_LABELS[m.invoice.category] }}
          </span>
          <span class="text-gray-500 truncate flex-1">{{ m.invoice.seller_name || m.invoice.invoice_number || m.invoice.id }}</span>
          <span class="text-gray-500 shrink-0">{{ m.invoice.travel_date || m.invoice.date }}</span>
          <span class="text-gray-800 shrink-0">¥{{ m.invoice.amount.toFixed(2) }}</span>
          <span class="text-primary-600 text-xs shrink-0">详情</span>
          <select :value="trip.id" @click.stop @change="handleMoveInvoice(m.invoice_id, ($event.target as HTMLSelectElement).value)"
                  class="text-xs border rounded px-1 py-0.5 shrink-0">
            <option v-for="t in otherTrips" :key="t.id" :value="t.id">出差 {{ t.destination || '未设置' }} {{ t.travelStart }}~{{ t.travelEnd }}</option>
            <option value="">移到待调整</option>
          </select>
        </div>
      </div>
    </div>

    <InvoiceDetailModal
      :visible="detailVisible"
      :invoice="detailInvoice"
      @close="detailVisible = false"
      @save="handleDetailSave"
    />

    <div class="flex items-center gap-1.5">
      <button @click="togglePreview" :title="previewing ? '收起预览' : '预览本趟报销单'"
              :aria-label="previewing ? '收起预览' : '预览本趟报销单'"
              class="w-8 h-8 rounded-lg border border-gray-300 hover:bg-gray-50 flex items-center justify-center">
        <AppIcon name="eye" :size="16" />
      </button>
      <ExportButton
        :match-results="trip.matches"
        :unmatched-invoice-ids="[]"
        :unmatched-payment-ids="[]"
        :form-info="formInfo"
        show-labels
      />
    </div>
    <div v-if="previewing && previewHtml" class="border border-gray-200 rounded-lg overflow-hidden">
      <iframe :srcdoc="previewHtml" class="w-full" style="min-height: 500px; border: none;" title="报销单预览" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import AppBadge from './ui/AppBadge.vue'
import AppIcon from './ui/AppIcon.vue'
import ReimbursementForm from './ReimbursementForm.vue'
import ExportButton from './ExportButton.vue'
import InvoiceDetailModal from './InvoiceDetailModal.vue'
import { useMatchStore } from '../stores/match'
import { useInvoiceStore } from '../stores/invoice'
import { toast } from '../composables/toast'
import type { Invoice, Trip } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass } from '../utils/category'

const props = defineProps<{
  trip: Trip
  index: number
  otherTrips: Trip[]
}>()

const emit = defineEmits<{
  (e: 'move', invoiceId: string, targetTripId: string | null): void
  (e: 'form-update', tripId: string, form: { destination: string; travelStart: string; travelEnd: string; hotelLevel: string }): void
}>()

const matchStore = useMatchStore()
const invoiceStore = useInvoiceStore()

const showInvoices = ref(false)
const detailVisible = ref(false)
const detailInvoice = ref<Invoice | null>(null)
const previewing = ref(false)
const previewHtml = ref<string | null>(null)

const tripTotal = computed(() => props.trip.matches.reduce((s, m) => s + m.invoice.amount, 0))

const formModel = computed(() => ({
  destination: props.trip.destination,
  travelStart: props.trip.travelStart,
  travelEnd: props.trip.travelEnd,
  hotelLevel: props.trip.hotelLevel,
}))

const formInfo = computed(() => ({
  name: '',
  department: '',
  destination: props.trip.destination,
  travelStart: props.trip.travelStart,
  travelEnd: props.trip.travelEnd,
  companions: 0,
  hotelLevel: props.trip.hotelLevel,
}))

function handleFormUpdate(form: { destination: string; travelStart: string; travelEnd: string; hotelLevel: string }) {
  emit('form-update', props.trip.id, form)
}

function handleMoveInvoice(invoiceId: string, targetTripId: string) {
  emit('move', invoiceId, targetTripId || null)
}

function openDetail(invoice: Invoice) {
  detailInvoice.value = invoice
  detailVisible.value = true
}

function handleDetailSave(updated: Invoice) {
  // 就地更新 match 与发票 store（趟内/待调整引用同一 MatchResult，自动同步）
  matchStore.updateMatchInvoice(updated)
  invoiceStore.updateInvoice(updated)
  detailVisible.value = false
}

async function togglePreview() {
  if (previewing.value) {
    previewing.value = false
    previewHtml.value = null
    return
  }
  try {
    const html = await matchStore.renderReimbursementHtml(formInfo.value, props.trip.matches)
    previewHtml.value = html
    previewing.value = true
  } catch (e) {
    console.error('预览失败:', e)
    toast('预览失败: ' + e, 'error')
  }
}
</script>
