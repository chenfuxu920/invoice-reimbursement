<template>
  <div class="max-w-4xl mx-auto px-5 py-6 pb-10">
    <div class="flex flex-wrap items-center justify-between gap-3 mb-6 animate-fade-in-up">
      <div>
        <h2 class="font-display text-2xl font-extrabold text-slate-900">打包导出</h2>
        <p class="text-sm text-slate-500 mt-1">按出差分趟归档，一键生成报销材料</p>
      </div>
    </div>

    <AppEmpty v-if="matchStore.matches.length === 0" icon="download" message="请先在核对匹配页完成发票与账单的匹配" class="animate-fade-in-up">
      <AppButton variant="primary" @click="$router.push('/match')">去核对匹配</AppButton>
    </AppEmpty>

    <template v-else>
      <!-- 匹配摘要（小芯片行） -->
      <div class="flex flex-wrap items-center gap-2 mb-6 animate-fade-in-up">
        <span class="chip bg-white text-slate-600 border border-slate-200 shadow-card"><CheckCircle2 :size="13" class="text-emerald-500" /> 已匹配 {{ matchStore.matches.length }}</span>
        <span v-if="matchStore.unmatchedInvoices.length" class="chip bg-amber-50 text-amber-700 border border-amber-200/70"><AlertTriangle :size="13" /> 未匹配发票 {{ matchStore.unmatchedInvoices.length }}</span>
        <span v-if="matchStore.unmatchedPayments.length" class="chip bg-slate-50 text-slate-600 border border-slate-200/70">未匹配支付 {{ matchStore.unmatchedPayments.length }}</span>
        <span class="chip bg-white text-slate-600 border border-slate-200 shadow-card"><Package :size="13" class="text-primary-600" /> 出差 {{ matchStore.trips.length }} 趟</span>
        <span v-if="stayMismatchCount" class="chip bg-amber-50 text-amber-700 border border-amber-200/70"><AlertTriangle :size="13" /> 住宿天数不符 {{ stayMismatchCount }} 趟</span>
      </div>

      <!-- 分趟工具栏：存在待调整票据时提供出发城市重匹配 -->
      <div v-if="hasUnassignedTickets"
           class="card p-5 mb-6 flex flex-wrap items-center gap-3 animate-fade-in-up">
        <div class="flex items-center gap-2">
          <label class="text-sm text-slate-600">出发城市</label>
          <input v-model="originInput" class="input !w-36 !py-1.5" placeholder="如：长沙" />
        </div>
        <AppButton variant="primary" @click="handleResegment">重新匹配行程</AppButton>
        <AppButton @click="handleResetAuto">恢复自动分趟</AppButton>
        <span v-if="matchStore.segmentOrigin" class="text-xs text-slate-400">
          当前按出发城市「{{ matchStore.segmentOrigin }}」分组
        </span>
      </div>

      <!-- 一键导出所有出差 -->
      <div v-if="matchStore.trips.length"
           class="card card-hover p-5 mb-6 flex flex-wrap items-center justify-between gap-4 animate-fade-in-up"
           style="animation-delay: 60ms">
        <div class="flex items-center gap-3 min-w-0">
          <span class="w-11 h-11 rounded-2xl bg-gradient-to-br from-emerald-500 to-teal-500 text-white shadow-glow-sm flex items-center justify-center shrink-0">
            <FileDown :size="20" />
          </span>
          <div class="min-w-0">
            <p class="font-display text-sm font-bold text-slate-800">一键导出所有出差</p>
            <p class="text-xs text-slate-400 mt-0.5">选择目录后，每一趟出差将导出为单独的文件（共 {{ matchStore.trips.length }} 趟）</p>
          </div>
        </div>
        <ExportButton
          :match-results="[]"
          :unmatched-invoice-ids="[]"
          :unmatched-payment-ids="[]"
          :form-info="batchFormInfo"
          :trips="matchStore.trips"
          show-labels
        />
      </div>

      <!-- 报销人信息（自动持久化，导出时填入报销材料） -->
      <div v-if="matchStore.trips.length"
           class="card p-5 mb-6 animate-fade-in-up"
           style="animation-delay: 120ms">
        <div class="flex items-center gap-2 mb-1">
          <ClipboardList :size="16" class="text-primary-600" />
          <h3 class="font-display text-sm font-bold text-slate-800">报销人信息</h3>
        </div>
        <p class="text-xs text-slate-400 mb-4">姓名、部门与同行人数将自动填入导出的报销材料</p>
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1.5">姓名</label>
            <input v-model="profile.name" class="input" placeholder="请输入姓名" />
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1.5">部门</label>
            <input v-model="profile.department" class="input" placeholder="请输入部门" />
          </div>
          <div>
            <label class="block text-sm font-medium text-slate-600 mb-1.5">同行人数</label>
            <input v-model.number="profile.companions" type="number" min="0" class="input" />
          </div>
        </div>
      </div>

      <!-- 分趟封面档案卡列表 -->
      <div class="space-y-6 mb-6">
        <TripCard
          v-for="(trip, idx) in matchStore.trips"
          :key="trip.id"
          :trip="trip"
          :index="idx + 1"
          :other-trips="otherTrips(trip)"
          :style="{ animationDelay: `${idx * 90}ms` }"
          class="animate-fade-in-up"
          @move="handleMove"
          @form-update="handleTripFormUpdate"
        />
      </div>

      <!-- 待调整区 -->
      <div v-if="matchStore.unassigned.length"
           class="card border-amber-200/80 bg-gradient-to-br from-amber-50/80 to-orange-50/40 p-5 mb-6 animate-fade-in-up">
        <div class="flex items-center gap-2 mb-1">
          <AlertTriangle :size="16" class="text-amber-500" />
          <h3 class="font-display text-base font-bold text-amber-800">待调整（{{ matchStore.unassigned.length }}）</h3>
        </div>
        <p class="text-xs text-amber-600/90 mb-4 leading-relaxed">
          以下发票已匹配支付记录，但尚未归入任何一趟出差，可移入某趟；票据可「新建出差」。
        </p>
        <div class="space-y-2">
          <div v-for="m in matchStore.unassigned" :key="m.invoice_id"
               class="flex flex-wrap items-center gap-2 bg-white/80 rounded-xl px-4 py-2.5 border border-amber-100 text-sm cursor-pointer hover:bg-white hover:shadow-card transition-all"
               @click="openDetail(m.invoice)">
            <span class="w-20 shrink-0 text-xs font-medium chip border" :class="getCategoryBadgeClass(m.invoice.category)">
              {{ CATEGORY_LABELS[m.invoice.category] }}
            </span>
            <span class="text-slate-500 truncate flex-1 min-w-24">{{ m.invoice.seller_name || m.invoice.invoice_number || m.invoice.id }}</span>
            <span class="text-slate-500 shrink-0">{{ m.invoice.travel_date || m.invoice.date }}</span>
            <span class="font-semibold text-slate-800 shrink-0 tabular-nums">¥{{ m.invoice.amount.toFixed(2) }}</span>
            <span class="text-primary-600 text-xs shrink-0">详情</span>
            <AppButton v-if="isTicket(m.invoice)" variant="primary" size="sm" @click.stop="handleCreateTrip(m)">
              <Plus :size="12" class="inline-block -mt-0.5" />新建出差
            </AppButton>
            <select @click.stop @change="handleMove(m.invoice_id, ($event.target as HTMLSelectElement).value)"
                    class="input-sm !w-auto shrink-0 cursor-pointer">
              <option value="" disabled selected>移到出差...</option>
              <option v-for="t in matchStore.trips" :key="t.id" :value="t.id">出差 {{ t.destination || '未设置' }} {{ t.travelStart }}~{{ t.travelEnd }}</option>
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
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { CheckCircle2, AlertTriangle, Package, FileDown, Plus, ClipboardList } from 'lucide-vue-next'
import { useMatchStore } from '../stores/match'
import { useInvoiceStore } from '../stores/invoice'
import TripCard from '../components/TripCard.vue'
import ExportButton from '../components/ExportButton.vue'
import InvoiceDetailModal from '../components/InvoiceDetailModal.vue'
import AppButton from '../components/ui/AppButton.vue'
import AppEmpty from '../components/ui/AppEmpty.vue'
import { toast } from '../composables/toast'
import { useProfile } from '../composables/profile'
import type { Invoice, MatchResult, Trip } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass } from '../utils/category'
import { analyzeStayDays } from '../utils/stay'

const matchStore = useMatchStore()
const invoiceStore = useInvoiceStore()

const { profile } = useProfile()

const originInput = ref('')
const detailVisible = ref(false)
const detailInvoice = ref<Invoice | null>(null)

function openDetail(invoice: Invoice) {
  detailInvoice.value = invoice
  detailVisible.value = true
}

function handleDetailSave(updated: Invoice) {
  matchStore.updateMatchInvoice(updated)
  invoiceStore.updateInvoice(updated)
  detailVisible.value = false
}

function isTicket(invoice: Invoice) {
  return invoice.category === 'Train' || invoice.category === 'Flight'
}

const batchFormInfo = computed(() => {
  const trips = matchStore.trips
  const starts = trips.map(t => t.travelStart).filter(Boolean).sort()
  const ends = trips.map(t => t.travelEnd).filter(Boolean).sort()
  return {
    name: profile.value.name,
    department: profile.value.department,
    destination: trips.map(t => t.destination).filter(Boolean).join('、') || '未设置',
    travelStart: starts[0] || '',
    travelEnd: ends[ends.length - 1] || '',
    companions: profile.value.companions,
    hotelLevel: '',
  }
})

const hasUnassignedTickets = computed(() =>
  matchStore.unassigned.some(m => isTicket(m.invoice))
)

// 住宿天数与行程对不上的趟数（摘要芯片提示，详情见对应出差卡片）
const stayMismatchCount = computed(() =>
  matchStore.trips.filter(t => analyzeStayDays(t)?.status === 'mismatch').length
)

function otherTrips(trip: Trip): Trip[] {
  return matchStore.trips.filter(t => t.id !== trip.id)
}

async function handleResegment() {
  const origin = originInput.value.trim()
  if (!origin) {
    toast('请先输入出发城市', 'info')
    return
  }
  try {
    await matchStore.resegment(matchStore.matches, origin)
    matchStore.segmentOrigin = origin
  } catch (e) {
    console.error('重新匹配失败:', e)
    toast('重新匹配失败: ' + e, 'error')
  }
}

async function handleResetAuto() {
  try {
    await matchStore.resegment(matchStore.matches, '')
    matchStore.segmentOrigin = ''
    originInput.value = ''
  } catch (e) {
    console.error('恢复自动分趟失败:', e)
    toast('恢复自动分趟失败: ' + e, 'error')
  }
}

function handleMove(invoiceId: string, targetTripId: string | null) {
  matchStore.moveToTrip(invoiceId, targetTripId)
}

function handleTripFormUpdate(tripId: string, form: { destination: string; travelStart: string; travelEnd: string; hotelLevel: string }) {
  const trip = matchStore.trips.find(t => t.id === tripId)
  if (!trip) return
  trip.destination = form.destination
  trip.travelStart = form.travelStart
  trip.travelEnd = form.travelEnd
  trip.hotelLevel = form.hotelLevel
}

function handleCreateTrip(match: MatchResult) {
  matchStore.createTripFromTicket(match)
}
</script>
