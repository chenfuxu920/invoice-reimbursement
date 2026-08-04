<template>
  <div class="max-w-4xl mx-auto">
    <h2 class="text-2xl font-bold mb-6">导出报销表</h2>

    <AppEmpty v-if="matchStore.matches.length === 0" icon="download" message="请先在匹配页面完成发票与账单的匹配" />

    <template v-else>
      <!-- 匹配摘要 -->
      <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 mb-6">
        <div class="grid grid-cols-1 sm:grid-cols-3 gap-4 text-center">
          <div>
            <p class="text-2xl font-bold text-primary-600 tabular-nums">{{ matchStore.matches.length }}</p>
            <p class="text-sm text-gray-500">已匹配</p>
          </div>
          <div>
            <p class="text-2xl font-bold text-amber-600 tabular-nums">{{ matchStore.unmatchedInvoices.length }}</p>
            <p class="text-sm text-gray-500">未匹配发票</p>
          </div>
          <div>
            <p class="text-2xl font-bold text-gray-600 tabular-nums">{{ matchStore.unmatchedPayments.length }}</p>
            <p class="text-sm text-gray-500">未匹配支付</p>
          </div>
        </div>
      </div>

      <!-- 分趟工具栏：存在待调整票据时提供出发城市重匹配 -->
      <div v-if="hasUnassignedTickets"
           class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 mb-6 flex flex-wrap items-center gap-3">
        <div class="flex items-center gap-2">
          <label class="text-sm text-gray-600">出发城市</label>
          <input v-model="originInput" class="w-32 border border-gray-300 rounded-lg px-2 py-1.5 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100"
                 placeholder="如：长沙" />
        </div>
        <AppButton variant="primary" @click="handleResegment">重新匹配行程</AppButton>
        <AppButton @click="handleResetAuto">恢复自动分趟</AppButton>
        <span v-if="matchStore.segmentOrigin" class="text-xs text-gray-400">
          当前按出发城市「{{ matchStore.segmentOrigin }}」分组
        </span>
      </div>

      <!-- 一键导出所有出差 -->
      <div v-if="matchStore.trips.length"
           class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 mb-6 flex items-center justify-between gap-3">
        <div>
          <p class="text-sm font-medium text-gray-700">一键导出所有出差</p>
          <p class="text-xs text-gray-400 mt-0.5">选择目录后，每一趟出差将导出为单独的文件（共 {{ matchStore.trips.length }} 趟）</p>
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

      <!-- 分趟列表 -->
      <div class="space-y-6 mb-6">
        <TripCard
          v-for="(trip, idx) in matchStore.trips"
          :key="trip.id"
          :trip="trip"
          :index="idx + 1"
          :other-trips="otherTrips(trip)"
          @move="handleMove"
          @form-update="handleTripFormUpdate"
        />
      </div>

      <!-- 待调整区 -->
      <div v-if="matchStore.unassigned.length" class="bg-amber-50 border border-amber-200 rounded-[10px] p-4 mb-6">
        <h3 class="text-sm font-medium text-amber-700 mb-1">待调整（{{ matchStore.unassigned.length }}）</h3>
        <p class="text-xs text-amber-500 mb-3">
          以下发票无法自动归入某趟出差（票据未配对成功或日期在行程之外），可移入某趟；票据可「新建出差」。
        </p>
        <div class="space-y-2">
          <div v-for="m in matchStore.unassigned" :key="m.invoice_id"
               class="flex items-center gap-2 bg-white rounded px-3 py-2 border border-amber-100 text-sm flex-wrap cursor-pointer hover:bg-amber-50"
               @click="openDetail(m.invoice)">
            <span class="w-20 shrink-0 text-xs font-medium" :class="getCategoryBadgeClass(m.invoice.category)">
              {{ CATEGORY_LABELS[m.invoice.category] }}
            </span>
            <span class="text-gray-500 truncate flex-1">{{ m.invoice.seller_name || m.invoice.invoice_number || m.invoice.id }}</span>
            <span class="text-gray-500 shrink-0">{{ m.invoice.travel_date || m.invoice.date }}</span>
            <span class="text-gray-800 shrink-0">¥{{ m.invoice.amount.toFixed(2) }}</span>
            <span class="text-primary-600 text-xs shrink-0">详情</span>
            <AppButton v-if="isTicket(m.invoice)" variant="primary" size="sm" @click.stop="handleCreateTrip(m)">
              <AppIcon name="plus" :size="12" class="inline-block mr-0.5 -mt-0.5" />新建出差
            </AppButton>
            <select @click.stop @change="handleMove(m.invoice_id, ($event.target as HTMLSelectElement).value)"
                    class="text-xs border rounded px-1 py-0.5 shrink-0">
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
import { useMatchStore } from '../stores/match'
import { useInvoiceStore } from '../stores/invoice'
import TripCard from '../components/TripCard.vue'
import ExportButton from '../components/ExportButton.vue'
import InvoiceDetailModal from '../components/InvoiceDetailModal.vue'
import AppButton from '../components/ui/AppButton.vue'
import AppIcon from '../components/ui/AppIcon.vue'
import AppEmpty from '../components/ui/AppEmpty.vue'
import { toast } from '../composables/toast'
import type { Invoice, MatchResult, Trip } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass } from '../utils/category'

const matchStore = useMatchStore()
const invoiceStore = useInvoiceStore()

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
    name: '',
    department: '',
    destination: trips.map(t => t.destination).filter(Boolean).join('、') || '未设置',
    travelStart: starts[0] || '',
    travelEnd: ends[ends.length - 1] || '',
    companions: 0,
    hotelLevel: '',
  }
})

const hasUnassignedTickets = computed(() =>
  matchStore.unassigned.some(m => isTicket(m.invoice))
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
