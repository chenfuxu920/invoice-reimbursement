<template>
  <div class="card overflow-hidden animate-fade-in-up">
    <!-- 封面区 -->
    <div class="relative overflow-hidden bg-gradient-to-br from-slate-900 via-primary-900 to-accent-700 px-6 py-6">
      <div class="absolute -top-14 -right-10 w-48 h-48 rounded-full bg-accent-500/20 pointer-events-none" />
      <div class="absolute -bottom-16 left-1/3 w-44 h-44 rounded-full bg-primary-500/20 pointer-events-none" />
      <div class="absolute top-5 right-6 opacity-10 text-white rotate-6 pointer-events-none">
        <MapPin :size="72" />
      </div>

      <div class="relative flex flex-wrap items-end justify-between gap-4">
        <div class="min-w-0">
          <div class="flex items-center gap-2 mb-2">
            <span class="chip bg-white/15 text-white border border-white/20">出差 {{ index }}</span>
            <span class="chip bg-white/10 text-white/80 border border-white/10">
              <CalendarDays :size="12" /> {{ trip.travelStart || '—' }} 至 {{ trip.travelEnd || '—' }}
            </span>
          </div>
          <h3 class="font-display text-2xl md:text-3xl font-extrabold text-white truncate">{{ trip.destination || '未设置目的地' }}</h3>
          <p class="text-sm text-white/70 mt-1.5">
            城市间交通 {{ trip.ticketIds.length }} · 发票 {{ trip.matches.length }}
          </p>
        </div>
        <div class="text-right shrink-0">
          <p class="text-xs text-white/60">可报销金额</p>
          <p class="font-display text-3xl md:text-4xl font-extrabold text-white tabular-nums">{{ displayTotal }}</p>
          <p v-if="reimbursableTotal !== null" class="text-[11px] text-white/50 mt-1">按当前报销标准计算</p>
          <p v-else-if="firstSettled" class="text-[11px] text-white/50 mt-1">按发票原始金额</p>
        </div>
      </div>
    </div>

    <!-- 报销表单 -->
    <div class="p-5">
      <ReimbursementForm :model-value="formModel" @update="handleFormUpdate" />

      <!-- 发票明细 -->
      <div class="mt-4 rounded-xl border border-slate-200 overflow-hidden">
        <button @click="showInvoices = !showInvoices" :aria-expanded="showInvoices"
                class="w-full flex items-center justify-between px-4 py-3 text-sm font-medium text-slate-600 hover:bg-slate-50 transition-colors">
          <span class="flex items-center gap-2">
            <Receipt :size="15" class="text-primary-600" />
            发票明细（{{ trip.matches.length }}）
          </span>
          <ChevronDown :size="15" class="text-slate-400 transition-transform duration-300" :class="{ 'rotate-180': showInvoices }" />
        </button>
        <Transition name="acc">
          <div v-if="showInvoices" class="divide-y divide-slate-100 border-t border-slate-100">
            <div v-for="m in trip.matches" :key="m.invoice_id"
                 class="flex flex-wrap items-center gap-2 px-4 py-2.5 text-sm hover:bg-slate-50 cursor-pointer transition-colors"
                 @click="openDetail(m.invoice)">
              <span class="w-20 shrink-0 text-xs font-medium chip border !py-0.5" :class="getCategoryBadgeClass(m.invoice.category)">
                {{ CATEGORY_LABELS[m.invoice.category] }}
              </span>
              <span class="text-slate-500 truncate flex-1 min-w-24">{{ m.invoice.seller_name || m.invoice.invoice_number || m.invoice.id }}</span>
              <span class="text-slate-400 shrink-0">{{ m.invoice.travel_date || m.invoice.date }}</span>
              <span class="font-semibold text-slate-800 shrink-0 tabular-nums">¥{{ m.invoice.amount.toFixed(2) }}</span>
              <span class="text-primary-600 text-xs shrink-0">详情</span>
              <select :value="trip.id" @click.stop @change="handleMoveInvoice(m.invoice_id, ($event.target as HTMLSelectElement).value)"
                      class="input-sm !w-auto shrink-0 cursor-pointer">
                <option v-for="t in otherTrips" :key="t.id" :value="t.id">出差 {{ t.destination || '未设置' }} {{ t.travelStart }}~{{ t.travelEnd }}</option>
                <option value="">移到待调整</option>
              </select>
            </div>
          </div>
        </Transition>
      </div>

      <!-- 操作区 -->
      <div class="mt-4 flex flex-wrap items-center gap-2">
        <AppButton variant="secondary" size="sm" @click="togglePreview" :title="previewing ? '收起预览' : '预览本趟报销单'">
          <Eye :size="14" />
          {{ previewing ? '收起预览' : '预览' }}
        </AppButton>
        <ExportButton
          :match-results="trip.matches"
          :unmatched-invoice-ids="[]"
          :unmatched-payment-ids="[]"
          :form-info="formInfo"
          show-labels
        />
      </div>
      <div v-if="previewing && previewHtml" class="mt-4 rounded-xl border border-slate-200 overflow-hidden animate-fade-in">
        <iframe :srcdoc="previewHtml" class="w-full" style="min-height: 500px; border: none;" title="报销单预览" />
      </div>
    </div>

    <InvoiceDetailModal
      :visible="detailVisible"
      :invoice="detailInvoice"
      @close="detailVisible = false"
      @save="handleDetailSave"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { watchDebounced } from '@vueuse/core'
import { invoke } from '@tauri-apps/api/core'
import { MapPin, CalendarDays, Receipt, ChevronDown, Eye } from 'lucide-vue-next'
import AppButton from './ui/AppButton.vue'
import ReimbursementForm from './ReimbursementForm.vue'
import ExportButton from './ExportButton.vue'
import InvoiceDetailModal from './InvoiceDetailModal.vue'
import { useMatchStore } from '../stores/match'
import { useInvoiceStore } from '../stores/invoice'
import { toast } from '../composables/toast'
import type { Invoice, Trip, ReimbursementForm as ReimbursementFormResult } from '../types'
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

// 可报销金额：按最新报销标准实时计算（后端 build_reimbursement_form）
const reimbursableTotal = ref<number | null>(null)
// 首次计算是否已结束（成功或失败），未成功过则回退显示发票原始合计
const firstSettled = ref(false)
// 请求序号：并发/竞态时只采用最后一次结果
let requestSeq = 0

const displayTotal = computed(() => {
  if (reimbursableTotal.value !== null) return '¥' + reimbursableTotal.value.toFixed(2)
  return firstSettled.value ? '¥' + tripTotal.value.toFixed(2) : '…'
})

async function refreshReimbursable() {
  const seq = ++requestSeq
  if (!props.trip.matches.length) {
    reimbursableTotal.value = 0
    firstSettled.value = true
    return
  }
  try {
    const form = await invoke<ReimbursementFormResult>('preview_reimbursement_form', {
      matchResults: props.trip.matches,
      name: '',
      department: '',
      destination: props.trip.destination,
      travelStart: props.trip.travelStart,
      travelEnd: props.trip.travelEnd,
      companions: 0,
      hotelLevel: props.trip.hotelLevel,
    })
    if (seq !== requestSeq) return // 丢弃过期结果
    reimbursableTotal.value = form.total_amount
  } catch (e) {
    // 失败保留旧值；从未成功过则回退显示原始合计。不 toast，避免导出页噪音
    console.error('计算可报销金额失败:', e)
  } finally {
    if (seq === requestSeq) firstSettled.value = true
  }
}

// 出差信息变化时防抖重算（避免输入过程中频繁调用后端）
watchDebounced(
  [
    () => props.trip.destination,
    () => props.trip.travelStart,
    () => props.trip.travelEnd,
    () => props.trip.hotelLevel,
    () => props.trip.matches.length,
  ],
  () => refreshReimbursable(),
  { debounce: 400 },
)

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
  // 发票金额/类别可能变化，立即重算可报销金额
  refreshReimbursable()
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

onMounted(refreshReimbursable)
</script>

<style scoped>
.acc-enter-active, .acc-leave-active { transition: all 0.25s ease; }
.acc-enter-from, .acc-leave-to { opacity: 0; transform: translateY(-4px); }
</style>
