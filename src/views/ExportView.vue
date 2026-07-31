<template>
  <div class="max-w-4xl mx-auto">
    <h2 class="text-2xl font-bold mb-6">导出报销表</h2>

    <div v-if="matchStore.matches.length === 0" class="text-center py-12 text-gray-400">
      请先在匹配页面完成发票与账单的匹配
    </div>

    <template v-else>
      <!-- 匹配摘要 -->
      <div class="bg-white rounded-lg border p-4 shadow-sm mb-6">
        <div class="grid grid-cols-3 gap-4 text-center">
          <div>
            <p class="text-2xl font-bold text-blue-600">{{ matchStore.matches.length }}</p>
            <p class="text-sm text-gray-500">已匹配</p>
          </div>
          <div>
            <p class="text-2xl font-bold text-orange-500">{{ matchStore.unmatchedInvoices.length }}</p>
            <p class="text-sm text-gray-500">未匹配发票</p>
          </div>
          <div>
            <p class="text-2xl font-bold text-gray-400">{{ matchStore.unmatchedPayments.length }}</p>
            <p class="text-sm text-gray-500">未匹配支付</p>
          </div>
        </div>
      </div>

      <!-- 分趟工具栏：存在待调整票据时提供出发城市重匹配 -->
      <div v-if="hasUnassignedTickets"
           class="bg-white rounded-lg border p-4 shadow-sm mb-6 flex flex-wrap items-center gap-3">
        <div class="flex items-center gap-2">
          <label class="text-sm text-gray-600">出发城市</label>
          <input v-model="originInput" class="w-32 border rounded px-2 py-1.5 text-sm focus:outline-none focus:border-blue-500"
                 placeholder="如：长沙" />
        </div>
        <button @click="handleResegment"
                class="px-3 py-2 rounded bg-green-500 text-white text-sm hover:bg-green-600 transition-colors">
          重新匹配行程
        </button>
        <button @click="handleResetAuto"
                class="px-3 py-2 rounded border text-sm hover:bg-gray-50 transition-colors">
          恢复自动分趟
        </button>
        <span v-if="matchStore.segmentOrigin" class="text-xs text-gray-400">
          当前按出发城市「{{ matchStore.segmentOrigin }}」分组
        </span>
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
          @preview="previewTrip(trip)"
        />
      </div>

      <!-- 待调整区 -->
      <div v-if="matchStore.unassigned.length" class="bg-orange-50 border border-orange-200 rounded-lg p-4 mb-6">
        <h3 class="text-sm font-medium text-orange-700 mb-1">待调整（{{ matchStore.unassigned.length }}）</h3>
        <p class="text-xs text-orange-500 mb-3">
          以下发票无法自动归入某趟出差（票据未配对成功或日期在行程之外），可移入某趟；票据可「新建出差」。
        </p>
        <div class="space-y-2">
          <div v-for="m in matchStore.unassigned" :key="m.invoice_id"
               class="flex items-center gap-2 bg-white rounded px-3 py-2 border border-orange-100 text-sm flex-wrap">
            <span class="w-20 shrink-0 text-xs font-medium" :class="getCategoryBadgeClass(m.invoice.category)">
              {{ CATEGORY_LABELS[m.invoice.category] }}
            </span>
            <span class="text-gray-500 truncate flex-1">{{ m.invoice.seller_name || m.invoice.invoice_number || m.invoice.id }}</span>
            <span class="text-gray-500 shrink-0">{{ m.invoice.travel_date || m.invoice.date }}</span>
            <span class="text-gray-800 shrink-0">¥{{ m.invoice.amount.toFixed(2) }}</span>
            <button v-if="isTicket(m.invoice)" @click="handleCreateTrip(m)"
                    class="text-xs px-2 py-1 rounded bg-blue-500 text-white hover:bg-blue-600 transition-colors shrink-0">
              新建出差
            </button>
            <select @change="handleMove(m.invoice_id, ($event.target as HTMLSelectElement).value)"
                    class="text-xs border rounded px-1 py-0.5 shrink-0">
              <option value="" disabled selected>移到出差...</option>
              <option v-for="t in matchStore.trips" :key="t.id" :value="t.id">出差 {{ t.destination || t.id }}</option>
            </select>
          </div>
        </div>
      </div>

      <!-- 报销单预览 -->
      <div v-if="matchStore.reimbursementHtml" class="border rounded-lg overflow-hidden mb-6">
        <div class="bg-gray-100 px-4 py-2 text-sm text-gray-600">
          <span>报销单预览{{ previewingTrip ? ' · ' + (previewingTrip.destination || previewingTrip.id) : '' }}</span>
        </div>
        <iframe
          :srcdoc="matchStore.reimbursementHtml"
          class="w-full"
          style="min-height: 600px; border: none;"
          title="报销单预览"
        />
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useMatchStore } from '../stores/match'
import TripCard from '../components/TripCard.vue'
import type { Invoice, MatchResult, Trip } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass } from '../utils/category'

const matchStore = useMatchStore()

const originInput = ref('')
const previewingTrip = ref<Trip | null>(null)

function isTicket(invoice: Invoice) {
  return invoice.category === 'Train' || invoice.category === 'Flight'
}

const hasUnassignedTickets = computed(() =>
  matchStore.unassigned.some(m => isTicket(m.invoice))
)

function otherTrips(trip: Trip): Trip[] {
  return matchStore.trips.filter(t => t.id !== trip.id)
}

async function handleResegment() {
  const origin = originInput.value.trim()
  if (!origin) {
    alert('请先输入出发城市')
    return
  }
  try {
    await matchStore.resegment(matchStore.matches, origin)
    matchStore.segmentOrigin = origin
  } catch (e) {
    console.error('重新匹配失败:', e)
    alert('重新匹配失败: ' + e)
  }
}

async function handleResetAuto() {
  try {
    await matchStore.resegment(matchStore.matches, '')
    matchStore.segmentOrigin = ''
    originInput.value = ''
  } catch (e) {
    console.error('恢复自动分趟失败:', e)
    alert('恢复自动分趟失败: ' + e)
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

async function previewTrip(trip: Trip) {
  previewingTrip.value = trip
  try {
    await matchStore.renderReimbursementHtml(
      {
        name: '',
        department: '',
        destination: trip.destination,
        travelStart: trip.travelStart,
        travelEnd: trip.travelEnd,
        companions: 0,
        hotelLevel: trip.hotelLevel,
      },
      trip.matches,
    )
  } catch (e) {
    console.error('预览失败:', e)
    previewingTrip.value = null
    alert('预览失败: ' + e)
  }
}
</script>
