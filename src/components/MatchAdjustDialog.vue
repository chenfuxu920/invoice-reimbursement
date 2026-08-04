<template>
  <div v-if="visible" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white rounded-[10px] shadow-2xl w-[560px] max-h-[85vh] flex flex-col">
      <!-- 标题 -->
      <div class="flex items-center justify-between px-5 py-4 border-b border-gray-100 shrink-0">
        <h2 class="text-base font-semibold text-gray-800">{{ hasItineraries ? '行程级配对' : '调整匹配' }}</h2>
        <button class="text-gray-400 hover:text-gray-600" aria-label="关闭" @click="$emit('close')">
          <AppIcon name="x" :size="16" />
        </button>
      </div>
      <!-- 内容区（可滚动） -->
      <div class="flex-1 overflow-auto px-5 py-4">
        <p class="text-sm text-gray-500 mb-3">
          {{ hasItineraries ? '为每条行程选择对应的支付记录：' : '为发票选择对应的支付记录：' }}
        </p>
        <div class="bg-primary-50 rounded p-3 mb-3">
          <p class="font-medium">{{ invoice?.invoice_number || '无编号' }}</p>
          <p class="text-sm">¥{{ invoice?.amount.toFixed(2) }} - {{ invoice?.seller_name }}</p>
        </div>

        <!-- 搜索 + 来源 + 排序 -->
        <div class="flex gap-2 mb-2">
          <input v-model="searchText" type="text" placeholder="搜索商户名、交易号..."
                 class="flex-1 border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100" />
          <select v-model="sourceFilter"
                  class="border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100">
            <option value="all">全部来源</option>
            <option value="Wechat">微信</option>
            <option value="Alipay">支付宝</option>
          </select>
          <select v-model="sortKey"
                  class="border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100">
            <option value="diff-asc">金额差异最小</option>
            <option value="time-diff-asc">时间差异最小</option>
            <option value="time-desc">时间↓</option>
            <option value="time-asc">时间↑</option>
            <option value="amount-desc">金额↓</option>
            <option value="amount-asc">金额↑</option>
          </select>
        </div>

        <!-- 时间范围 + 时间接近 -->
        <div class="flex items-center gap-2 mb-2 text-sm">
          <span class="text-gray-500 shrink-0 w-10">时间</span>
          <input v-model="timeStart" type="date"
                 class="flex-1 min-w-0 border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100" />
          <span class="text-gray-400">~</span>
          <input v-model="timeEnd" type="date"
                 class="flex-1 min-w-0 border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100" />
          <label class="flex items-center gap-1 shrink-0 text-xs text-gray-600 cursor-pointer select-none"
                 :class="{ 'text-primary-600 font-medium': onlyCloseTime }">
            <input type="checkbox" v-model="onlyCloseTime" class="cursor-pointer" />
            时间接近
          </label>
        </div>

        <!-- 金额范围 + 金额接近 -->
        <div class="flex items-center gap-2 mb-2 text-sm">
          <span class="text-gray-500 shrink-0 w-10">金额</span>
          <input v-model="amountMin" type="number" min="0" placeholder="最小"
                 class="flex-1 min-w-0 border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100" />
          <span class="text-gray-400">~</span>
          <input v-model="amountMax" type="number" min="0" placeholder="最大"
                 class="flex-1 min-w-0 border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100" />
          <label class="flex items-center gap-1 shrink-0 text-xs text-gray-600 cursor-pointer select-none"
                 :class="{ 'text-primary-600 font-medium': onlyCloseAmount }">
            <input type="checkbox" v-model="onlyCloseAmount" class="cursor-pointer" />
            金额接近
          </label>
        </div>

        <!-- 交易类型 + 支付方式 + 退款/优惠 -->
        <div class="flex gap-2 mb-2">
          <select v-model="categoryFilter"
                  class="flex-1 min-w-0 border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100">
            <option value="all">全部类型</option>
            <option v-for="c in availableCategories" :key="c" :value="c">{{ c }}</option>
          </select>
          <select v-model="paymentMethodFilter"
                  class="flex-1 min-w-0 border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100">
            <option value="all">全部方式</option>
            <option v-for="m in availablePaymentMethods" :key="m" :value="m">{{ m }}</option>
          </select>
          <select v-model="refundFilter"
                  class="flex-1 min-w-0 border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100">
            <option value="all">退款/优惠</option>
            <option value="refund">有退款</option>
            <option value="discount">有优惠</option>
            <option value="none">无退款优惠</option>
          </select>
        </div>

        <!-- 清除所有过滤 + 计数 -->
        <div class="mb-2 flex items-center justify-between">
          <button v-if="hasActiveFilter" @click="clearFilters" class="text-xs text-primary-500 hover:text-primary-700">清除所有过滤</button>
          <span v-else></span>
          <span class="text-xs text-gray-400">
            {{ hasItineraries
              ? `已配对 ${pairedCount}/${invoice?.itineraries.length} 条行程`
              : `共 ${filteredPayments.length} 条 · 已选 ${selectedIds.size} 条` }}
          </span>
        </div>

        <!-- 行程级配对区（有行程单时） -->
        <div v-if="hasItineraries" class="space-y-2">
          <div v-for="(itin, idx) in invoice?.itineraries" :key="idx"
               class="p-2 rounded border"
               :class="itineraryPairs[idx] ? 'border-primary-300 bg-primary-50/40' : 'border-gray-200'">
            <div class="flex items-center justify-between gap-2">
              <div class="flex-1 min-w-0 text-sm">
                <p class="font-medium truncate">
                  <span class="text-gray-400 mr-1">#{{ idx + 1 }}</span>
                  {{ itin.provider || '未知' }} · {{ itin.pickup }} → {{ itin.dropoff }}
                </p>
                <p class="text-xs text-gray-400 mt-0.5">
                  {{ itin.date_time }} · 行程 ¥{{ itin.amount.toFixed(2) }}
                </p>
              </div>
              <select :value="itineraryPairs[idx] || ''"
                      @change="setItineraryPair(idx, ($event.target as HTMLSelectElement).value)"
                      class="shrink-0 max-w-[200px] border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100">
                <option value="">未选择</option>
                <option v-for="p in candidatesForItinerary(idx)" :key="p.id" :value="p.id">
                  ¥{{ p.amount.toFixed(2) }} · {{ p.merchant_name }} · {{ formatTime(p.transaction_time) }}
                </option>
              </select>
            </div>
            <!-- 选中后的差异展示 -->
            <div v-if="getSelectedPayment(idx)" class="text-xs mt-1 flex items-center gap-3 flex-wrap">
              <span class="text-gray-500">{{ getSelectedPayment(idx)!.merchant_name }}</span>
              <span class="text-gray-300">|</span>
              <span :class="itineraryAmountDiff(idx) <= 1 ? 'text-green-500' : 'text-orange-500'">
                金额差异 ¥{{ itineraryAmountDiff(idx).toFixed(2) }}
              </span>
              <span class="text-gray-300">|</span>
              <span :class="itineraryTimeDiffHours(idx) <= 12 ? 'text-green-500' : 'text-orange-500'">
                时间差异 {{ itineraryTimeDiffLabel(idx) }}
              </span>
            </div>
          </div>
          <div v-if="filteredPayments.length === 0" class="text-center py-4 text-sm text-gray-400">无可用支付记录</div>
        </div>

        <!-- 普通勾选列表（无行程单时） -->
        <div v-else class="space-y-2">
          <label v-for="p in filteredPayments" :key="p.id"
                 class="flex items-center gap-2 p-2 rounded border cursor-pointer hover:bg-gray-50"
                 :class="selectedIds.has(p.id) ? 'border-primary-500 bg-primary-50' : 'border-gray-200'">
            <input type="checkbox" :checked="selectedIds.has(p.id)" @change="togglePayment(p.id)" />
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <p class="text-sm font-medium truncate">{{ p.merchant_name }}</p>
                <span class="shrink-0 text-xs px-1.5 py-0.5 rounded"
                      :class="p.source === 'Wechat' ? 'bg-green-100 text-green-700' : 'bg-primary-100 text-primary-700'">
                  {{ p.source === 'Wechat' ? '微信' : '支付宝' }}
                </span>
                <span v-if="p.category" class="shrink-0 text-xs text-gray-400">{{ p.category }}</span>
              </div>
              <div class="flex items-center gap-2 text-xs mt-0.5 flex-wrap">
                <span class="text-gray-700 font-medium">¥{{ p.amount.toFixed(2) }}</span>
                <template v-if="p.refund_amount > 0 || p.discount > 0">
                  <span class="text-gray-300">|</span>
                  <span v-if="p.refund_amount > 0" class="text-red-400">退款 ¥{{ p.refund_amount.toFixed(2) }}</span>
                  <span v-if="p.refund_amount > 0 && p.discount > 0" class="text-gray-300"> </span>
                  <span v-if="p.discount > 0" class="text-green-400">优惠 ¥{{ p.discount.toFixed(2) }}</span>
                </template>
                <span class="text-gray-300">|</span>
                <span class="text-gray-400">{{ formatTime(p.transaction_time) }}</span>
                <span v-if="invoice" class="text-gray-300">|</span>
                <span v-if="invoice" class="text-orange-400" :title="'与发票金额的差异'">
                  差异 ¥{{ amountDiff(p).toFixed(2) }}
                </span>
              </div>
            </div>
            <span v-if="currentPaymentIds.has(p.id)" class="shrink-0 text-xs text-primary-500">当前匹配</span>
          </label>
          <div v-if="filteredPayments.length === 0" class="text-center py-4 text-sm text-gray-400">无匹配结果</div>
        </div>
      </div>
      <!-- 按钮区（固定底部，始终可见） -->
      <div class="px-5 py-3 border-t border-gray-100 flex justify-end gap-2 shrink-0">
        <AppButton @click="$emit('close')">取消</AppButton>
        <AppButton variant="primary" :disabled="!canConfirm" @click="confirmMatch">确认匹配</AppButton>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { Invoice, PaymentRecord, ItineraryPaymentPair } from '../types'
import AppButton from './ui/AppButton.vue'
import AppIcon from './ui/AppIcon.vue'

type SortKey = 'time-desc' | 'time-asc' | 'amount-desc' | 'amount-asc' | 'diff-asc' | 'time-diff-asc'
type RefundFilter = 'all' | 'refund' | 'discount' | 'none'

const props = defineProps<{
  visible: boolean
  invoice: Invoice | null
  currentPayments: PaymentRecord[]
  currentPairs: ItineraryPaymentPair[]
  availablePayments: PaymentRecord[]
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'confirm', invoice: Invoice, paymentIds: string[], itineraryPaymentPairs: ItineraryPaymentPair[]): void
}>()

const selectedIds = ref<Set<string>>(new Set())
// 行程级配对：itinerary_index → payment_id
const itineraryPairs = ref<Record<number, string>>({})
const searchText = ref('')
const sourceFilter = ref<'all' | 'Wechat' | 'Alipay'>('all')
const timeStart = ref('')
const timeEnd = ref('')
const sortKey = ref<SortKey>('diff-asc')
const amountMin = ref('')
const amountMax = ref('')
const onlyCloseAmount = ref(false)
const onlyCloseTime = ref(false)
const categoryFilter = ref('all')
const paymentMethodFilter = ref('all')
const refundFilter = ref<RefundFilter>('all')

const currentPaymentIds = computed(() => new Set(props.currentPayments.map(p => p.id)))

const hasItineraries = computed(() => (props.invoice?.itineraries.length ?? 0) > 0)

const allPayments = computed(() => {
  return [...props.currentPayments, ...props.availablePayments]
})

const availableCategories = computed(() => {
  const set = new Set<string>()
  allPayments.value.forEach(p => { if (p.category) set.add(p.category) })
  return Array.from(set).sort()
})

const availablePaymentMethods = computed(() => {
  const set = new Set<string>()
  allPayments.value.forEach(p => { if (p.payment_method) set.add(p.payment_method) })
  return Array.from(set).sort()
})

const hasActiveFilter = computed(() => {
  return !!searchText.value
    || sourceFilter.value !== 'all'
    || !!timeStart.value
    || !!timeEnd.value
    || !!amountMin.value
    || !!amountMax.value
    || onlyCloseAmount.value
    || onlyCloseTime.value
    || categoryFilter.value !== 'all'
    || paymentMethodFilter.value !== 'all'
    || refundFilter.value !== 'all'
})

const pairedCount = computed(() => {
  return Object.values(itineraryPairs.value).filter(v => v).length
})

const canConfirm = computed(() => {
  if (hasItineraries.value) return pairedCount.value > 0
  return selectedIds.value.size > 0
})

// ── 全局硬过滤（搜索/来源/时间范围/金额范围/交易类型/支付方式/退款优惠）──
// 对所有候选生效，行程与非行程模式共享。
function passesGlobalFilter(p: PaymentRecord): boolean {
  if (sourceFilter.value !== 'all' && p.source !== sourceFilter.value) return false
  if (timeStart.value || timeEnd.value) {
    const d = p.transaction_time.slice(0, 10)
    if (timeStart.value && d < timeStart.value) return false
    if (timeEnd.value && d > timeEnd.value) return false
  }
  if (searchText.value) {
    const q = searchText.value.toLowerCase()
    if (!p.merchant_name.toLowerCase().includes(q)
      && !p.transaction_id.toLowerCase().includes(q)) return false
  }
  const min = amountMin.value ? parseFloat(amountMin.value) : null
  const max = amountMax.value ? parseFloat(amountMax.value) : null
  if (min !== null && p.amount < min) return false
  if (max !== null && p.amount > max) return false
  if (categoryFilter.value !== 'all' && p.category !== categoryFilter.value) return false
  if (paymentMethodFilter.value !== 'all' && p.payment_method !== paymentMethodFilter.value) return false
  if (refundFilter.value === 'refund' && p.refund_amount <= 0) return false
  if (refundFilter.value === 'discount' && p.discount <= 0) return false
  if (refundFilter.value === 'none' && (p.refund_amount > 0 || p.discount > 0)) return false
  return true
}

const globalFilteredPayments = computed(() => allPayments.value.filter(passesGlobalFilter))

// 非行程模式：差异/接近针对发票
function amountDiff(p: PaymentRecord): number {
  if (!props.invoice) return 0
  return Math.abs(p.amount - props.invoice.amount)
}

/// 应用"接近"过滤 + 排序。amountTarget/timeTargetMs 为基准（行程模式传行程值，非行程传发票值）。
function applyCloseAndSort(
  list: PaymentRecord[],
  amountTarget: number,
  timeTargetMs: number | null,
): PaymentRecord[] {
  const closeAmountThreshold = Math.max(amountTarget * 0.1, 10)
  const closeTimeThresholdDays = 7
  let r = list
  if (onlyCloseAmount.value) {
    r = r.filter(p => Math.abs(p.amount - amountTarget) <= closeAmountThreshold)
  }
  if (onlyCloseTime.value && timeTargetMs != null) {
    r = r.filter(p => {
      const pt = parseTimeToMs(p.transaction_time)
      if (pt == null) return false
      return Math.abs(pt - timeTargetMs) / (1000 * 60 * 60 * 24) <= closeTimeThresholdDays
    })
  }
  const sorted = [...r]
  switch (sortKey.value) {
    case 'time-desc':
      sorted.sort((a, b) => b.transaction_time.localeCompare(a.transaction_time)); break
    case 'time-asc':
      sorted.sort((a, b) => a.transaction_time.localeCompare(b.transaction_time)); break
    case 'amount-desc':
      sorted.sort((a, b) => b.amount - a.amount); break
    case 'amount-asc':
      sorted.sort((a, b) => a.amount - b.amount); break
    case 'diff-asc':
      sorted.sort((a, b) => Math.abs(a.amount - amountTarget) - Math.abs(b.amount - amountTarget)); break
    case 'time-diff-asc':
      if (timeTargetMs != null) {
        sorted.sort((a, b) => {
          const ta = parseTimeToMs(a.transaction_time)
          const tb = parseTimeToMs(b.transaction_time)
          if (ta == null || tb == null) return 0
          return Math.abs(ta - timeTargetMs) - Math.abs(tb - timeTargetMs)
        })
      }
      break
  }
  return sorted
}

// 非行程模式列表
const filteredPayments = computed(() => {
  if (hasItineraries.value) return globalFilteredPayments.value
  const amountTarget = props.invoice?.amount ?? 0
  const timeTargetMs = props.invoice?.date ? parseTimeToMs(props.invoice.date) : null
  return applyCloseAndSort(globalFilteredPayments.value, amountTarget, timeTargetMs)
})

// ── 行程级配对辅助 ──

function getSelectedPayment(idx: number): PaymentRecord | undefined {
  const id = itineraryPairs.value[idx]
  return id ? allPayments.value.find(p => p.id === id) : undefined
}

/// 某条行程的可选支付：全局过滤 + 针对该行程的接近过滤/排序 + 排除已被其他行程选中。
/// 金额接近/时间接近/差异排序均以该行程的 amount 和 date_time 为基准。
function candidatesForItinerary(idx: number): PaymentRecord[] {
  const itin = props.invoice?.itineraries[idx]
  if (!itin) return []
  const amountTarget = itin.amount
  const timeTargetMs = parseTimeToMs(itin.date_time)
  const list = applyCloseAndSort(globalFilteredPayments.value, amountTarget, timeTargetMs)
  const usedByOthers = new Set<string>()
  for (const [k, v] of Object.entries(itineraryPairs.value)) {
    if (Number(k) !== idx && v) usedByOthers.add(v)
  }
  return list.filter(p => !usedByOthers.has(p.id))
}

function setItineraryPair(idx: number, paymentId: string) {
  itineraryPairs.value = { ...itineraryPairs.value, [idx]: paymentId }
}

function itineraryAmountDiff(idx: number): number {
  const itin = props.invoice?.itineraries[idx]
  const pay = getSelectedPayment(idx)
  if (!itin || !pay) return 0
  return Math.abs(pay.amount - itin.amount)
}

function itineraryTimeDiffHours(idx: number): number {
  const itin = props.invoice?.itineraries[idx]
  const pay = getSelectedPayment(idx)
  if (!itin || !pay) return Infinity
  const it = parseTimeToMs(itin.date_time)
  const pt = parseTimeToMs(pay.transaction_time)
  if (it == null || pt == null) return Infinity
  return Math.abs(pt - it) / (1000 * 60 * 60)
}

function itineraryTimeDiffLabel(idx: number): string {
  const itin = props.invoice?.itineraries[idx]
  const pay = getSelectedPayment(idx)
  if (!itin || !pay) return '未知'
  const it = parseTimeToMs(itin.date_time)
  const pt = parseTimeToMs(pay.transaction_time)
  if (it == null || pt == null) return '未知'
  return formatDuration(Math.abs(pt - it))
}

/// 将毫秒差格式化为具体时长：如 "35分钟"、"2小时15分钟"、"1天3小时"
function formatDuration(ms: number): string {
  const totalMin = Math.round(ms / (1000 * 60))
  if (totalMin < 1) return '0分钟'
  const days = Math.floor(totalMin / (60 * 24))
  const hours = Math.floor((totalMin % (60 * 24)) / 60)
  const mins = totalMin % 60
  const parts: string[] = []
  if (days > 0) parts.push(`${days}天`)
  if (hours > 0) parts.push(`${hours}小时`)
  if (mins > 0) parts.push(`${mins}分钟`)
  return parts.join('') || '0分钟'
}

function parseTimeToMs(t: string): number | null {
  // 支持 "YYYY-MM-DD HH:MM[:SS]" / "YYYY-MM-DD" / "MM-DD HH:MM[:SS]" / "MM-DD"
  // （行程单 OCR 常产出无年份格式如 "04-25 08:48"）
  const s = t.trim().replace(/:+$/, '').trim() // 容错尾部 ":" 如 "04-22 21:"
  // 带年份 "YYYY-MM-DD ..."
  if (s.length >= 10 && s[4] === '-') {
    const d = new Date(s.slice(0, 16).replace(' ', 'T'))
    if (!isNaN(d.getTime())) return d.getTime()
    const d2 = new Date(s.slice(0, 10))
    return isNaN(d2.getTime()) ? null : d2.getTime()
  }
  // 无年份 "MM-DD ..." → 优先用发票开票日期的年份（通常与行程同年），
  // 其次当前年/去年兜底
  if (s.length >= 5 && s[2] === '-' && /^\d{2}-\d{2}/.test(s)) {
    const invoiceYear = props.invoice?.date ? props.invoice.date.slice(0, 4) : ''
    const candidates = invoiceYear
      ? [invoiceYear, String(new Date().getFullYear()), String(new Date().getFullYear() - 1)]
      : [String(new Date().getFullYear()), String(new Date().getFullYear() - 1)]
    for (const y of candidates) {
      const withYear = `${y}-${s}`
      const d = new Date(withYear.slice(0, 16).replace(' ', 'T'))
      if (!isNaN(d.getTime())) return d.getTime()
      const d2 = new Date(withYear.slice(0, 10))
      if (!isNaN(d2.getTime())) return d2.getTime()
    }
  }
  return null
}

watch(() => props.visible, (v) => {
  if (v) {
    selectedIds.value = new Set(props.currentPayments.map(p => p.id))
    // 初始化行程配对：优先用现有 pairs，回退按 currentPayments 顺序
    const initPairs: Record<number, string> = {}
    if (props.invoice?.itineraries.length) {
      if (props.currentPairs?.length) {
        props.currentPairs.forEach(p => { initPairs[p.itinerary_index] = p.payment_id })
      } else {
        props.currentPayments.forEach((p, i) => {
          if (i < props.invoice!.itineraries.length) initPairs[i] = p.id
        })
      }
    }
    itineraryPairs.value = initPairs
    searchText.value = ''
    sourceFilter.value = 'all'
    timeStart.value = ''
    timeEnd.value = ''
    sortKey.value = 'diff-asc'
    amountMin.value = ''
    amountMax.value = ''
    onlyCloseAmount.value = false
    onlyCloseTime.value = false
    categoryFilter.value = 'all'
    paymentMethodFilter.value = 'all'
    refundFilter.value = 'all'
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

function clearFilters() {
  searchText.value = ''
  sourceFilter.value = 'all'
  timeStart.value = ''
  timeEnd.value = ''
  amountMin.value = ''
  amountMax.value = ''
  onlyCloseAmount.value = false
  onlyCloseTime.value = false
  categoryFilter.value = 'all'
  paymentMethodFilter.value = 'all'
  refundFilter.value = 'all'
}

function confirmMatch() {
  if (!props.invoice || !canConfirm.value) return
  if (hasItineraries.value) {
    const pairs: ItineraryPaymentPair[] = []
    const ids: string[] = []
    for (const [k, v] of Object.entries(itineraryPairs.value)) {
      if (v) {
        const idx = Number(k)
        pairs.push({ itinerary_index: idx, payment_id: v })
        if (!ids.includes(v)) ids.push(v)
      }
    }
    if (ids.length === 0) return
    emit('confirm', props.invoice, ids, pairs)
  } else {
    emit('confirm', props.invoice, Array.from(selectedIds.value), [])
  }
}
</script>
