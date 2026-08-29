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
        <div class="flex items-center gap-4 shrink-0">
          <!-- 市内交通标准使用率圆环 -->
          <button v-if="ctRing" type="button"
                  class="shrink-0 rounded-full focus:outline-none focus-visible:ring-2 focus-visible:ring-white/80 transition-transform duration-200"
                  :class="ctRing.over ? 'text-rose-400 cursor-pointer hover:scale-105' : 'text-emerald-400 cursor-default'"
                  :title="ctRing.title" :aria-label="ctRing.title" :aria-expanded="showOverage"
                  @click="ctRing.over && toggleOverage()">
            <DonutChart :percent="ctRing.rate" :tone="ctRing.over ? 'over' : 'ok'" :size="64" :stroke="7">
              <span class="font-display text-xs font-bold text-white tabular-nums leading-none">{{ Math.round(ctRing.rate) }}%</span>
            </DonutChart>
            <span class="block text-center text-[9px] leading-none text-white/60 mt-1.5 whitespace-nowrap">市内交通</span>
          </button>
          <!-- 住宿标准使用率圆环 -->
          <button v-if="hotelRing" type="button"
                  class="shrink-0 rounded-full focus:outline-none focus-visible:ring-2 focus-visible:ring-white/80 transition-transform duration-200"
                  :class="hotelRing.over ? 'text-rose-400 cursor-pointer hover:scale-105' : 'text-emerald-400 cursor-default'"
                  :title="hotelRing.title" :aria-label="hotelRing.title" :aria-expanded="showOverage"
                  @click="hotelRing.over && toggleOverage()">
            <DonutChart :percent="hotelRing.rate" :tone="hotelRing.over ? 'over' : 'ok'" :size="64" :stroke="7">
              <span class="font-display text-xs font-bold text-white tabular-nums leading-none">{{ Math.round(hotelRing.rate) }}%</span>
            </DonutChart>
            <span class="block text-center text-[9px] leading-none text-white/60 mt-1.5 whitespace-nowrap">住宿</span>
          </button>
          <div class="text-right">
            <p class="text-xs text-white/60">可报销金额</p>
            <p class="font-display text-3xl md:text-4xl font-extrabold text-white tabular-nums">{{ displayTotal }}</p>
            <button v-if="analysis && analysis.over" type="button" @click="toggleOverage"
                    class="mt-1 text-[11px] font-medium text-rose-200 underline decoration-rose-300/50 underline-offset-2 transition-colors hover:text-white">
              超出标准 ¥{{ fmt(analysis.overTotal) }}，点击查看
            </button>
            <p v-else-if="formResult !== null" class="text-[11px] text-white/50 mt-1">按当前报销标准计算</p>
            <p v-else-if="firstSettled" class="text-[11px] text-white/50 mt-1">按发票原始金额</p>
          </div>
        </div>
      </div>
    </div>

    <!-- 报销表单 -->
    <div class="p-5">
      <ReimbursementForm :model-value="formModel" @update="handleFormUpdate" />

      <!-- 住宿天数与行程天数核对（有住宿发票且可核对时才出现） -->
      <div v-if="stayCheck"
           class="mt-4 rounded-xl border px-4 py-3 flex items-start gap-2.5"
           :class="stayCheck.status === 'mismatch' ? 'border-amber-200/70 bg-amber-50/60' : 'border-slate-200 bg-slate-50'">
        <AlertTriangle v-if="stayCheck.status === 'mismatch'" :size="15" class="text-amber-500 shrink-0 mt-0.5" />
        <BedDouble v-else :size="15" class="text-slate-400 shrink-0 mt-0.5" />
        <p class="text-xs leading-relaxed" :class="stayCheck.status === 'mismatch' ? 'text-amber-800' : 'text-slate-600'">
          <template v-if="stayCheck.status === 'mismatch'">
            住宿天数与行程不符：行程 <span class="font-semibold tabular-nums">{{ stayCheck.tripDays }}</span> 天应为
            <span class="font-semibold tabular-nums">{{ stayCheck.expectedNights }}</span> 晚住宿，发票合计
            <span class="font-semibold tabular-nums">{{ stayCheck.nights }}</span> 晚，请核对行程日期或发票的入住/离店信息。
          </template>
          <template v-else>
            行程 <span class="font-semibold tabular-nums">{{ stayCheck.tripDays }}</span> 天应为
            <span class="font-semibold tabular-nums">{{ stayCheck.expectedNights }}</span> 晚住宿，但
            <span class="font-semibold tabular-nums">{{ stayCheck.unknownCount }}</span> 张住宿发票缺少入住/离店信息<template v-if="stayCheck.nights > 0">（已识别 {{ stayCheck.nights }} 晚）</template>，无法核对住宿天数。
          </template>
        </p>
      </div>

      <!-- 费用超标分析（存在超标时出现） -->
      <div v-if="analysis && analysis.over" ref="overagePanel"
           class="mt-4 rounded-xl border border-rose-200/70 bg-rose-50/50 overflow-hidden">
        <button type="button" :aria-expanded="showOverage"
                class="w-full flex flex-wrap items-center justify-between gap-2 px-4 py-3 text-sm font-semibold text-rose-800 hover:bg-rose-50 transition-colors"
                @click="toggleOverage">
          <span class="flex items-center gap-2">
            <AlertTriangle :size="15" class="text-rose-500" />
            费用超标分析
          </span>
          <span class="flex flex-wrap items-center gap-1.5">
            <span v-if="analysis.cityTransport?.over" class="chip bg-white text-rose-700 border border-rose-200 !py-0.5">市内交通 超 ¥{{ fmt(analysis.cityTransport.overAmount) }}</span>
            <span v-if="analysis.hotel?.over" class="chip bg-white text-rose-700 border border-rose-200 !py-0.5">住宿 超 ¥{{ fmt(analysis.hotel.overAmount) }}</span>
            <ChevronDown :size="15" class="text-rose-400 transition-transform duration-300" :class="{ 'rotate-180': showOverage }" />
          </span>
        </button>
        <Transition name="acc">
          <div v-if="showOverage" class="border-t border-rose-100 divide-y divide-rose-100">
            <!-- 市内交通费超标 -->
            <div v-if="analysis.cityTransport?.over" class="p-4">
              <div class="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5">
                <h4 class="flex items-center gap-1.5 text-sm font-bold text-rose-800">
                  <CarTaxiFront :size="15" class="text-rose-500" />
                  市内交通费超标 ¥{{ fmt(analysis.cityTransport.overAmount) }}
                </h4>
                <p class="text-xs text-slate-500 tabular-nums">
                  实际 ¥{{ fmt(analysis.cityTransport.actual) }} ｜ 标准 ¥{{ fmt(analysis.cityTransport.standard) }}（{{ fmtStd(analysis.cityTransport.dailyStd) }} 元/天 × {{ analysis.cityTransport.days }} 天）
                </p>
              </div>
              <p class="mt-1.5 text-xs leading-relaxed text-slate-500">
                超出部分不予报销。市内交通费按总额封顶，只需由同行人承接合计不低于超额的部分即可，以下两组建议均按移交金额最小计算，二选一使用。
              </p>

              <!-- 提示一：已开好发票 → 整张移交（支持手动挑选） -->
              <div class="mt-3">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <p class="flex items-center gap-1.5 text-xs font-semibold text-slate-600">
                    <Receipt :size="13" class="text-rose-500" />
                    可整张移交的发票
                    <span v-if="!manualPicking" class="font-normal text-slate-400">（{{ analysis.cityTransport.suggestedInvoices.length }} 张 · 合计 ¥{{ fmt(analysis.cityTransport.suggestedInvoicesTotal) }}）</span>
                    <span v-else class="font-normal text-slate-400">（共 {{ transferCandidates.length }} 张可挑）</span>
                  </p>
                  <button v-if="!manualPicking" type="button"
                          class="text-[11px] font-medium text-primary-600 hover:text-primary-700 transition-colors"
                          @click="enterManualPick">手动挑选</button>
                  <button v-else type="button"
                          class="text-[11px] font-medium text-slate-400 hover:text-slate-600 transition-colors"
                          @click="cancelManualPick">退出挑选</button>
                </div>
                <p class="mt-0.5 text-[11px] text-slate-400">
                  {{ manualPicking
                    ? '勾选要移交给同行人的发票，确认后为其单独生成一份移交报销单。'
                    : '发票已开具时无需重新开票，直接将以下整张发票交由同行人报销。' }}
                </p>
                <div class="mt-1.5 space-y-1">
                  <template v-if="!manualPicking">
                    <div v-for="inv in analysis.cityTransport.suggestedInvoices" :key="inv.invoiceId"
                         class="flex flex-wrap items-center gap-2 rounded-lg bg-white px-3 py-1.5 text-xs">
                      <span class="text-slate-600 truncate flex-1 min-w-24">{{ inv.label }}</span>
                      <span class="text-slate-400 shrink-0 tabular-nums">{{ inv.date }}</span>
                      <span class="font-semibold text-rose-700 shrink-0 tabular-nums">¥{{ fmt(inv.amount) }}</span>
                    </div>
                  </template>
                  <template v-else>
                    <label v-for="m in transferCandidates" :key="m.invoice_id"
                           class="flex flex-wrap items-center gap-2 rounded-lg bg-white px-3 py-1.5 text-xs cursor-pointer transition-shadow hover:shadow-card"
                           :class="selectedTransferIds.includes(m.invoice_id) ? 'ring-1 ring-primary-300' : ''">
                      <input type="checkbox" class="accent-primary-600 shrink-0"
                             :checked="selectedTransferIds.includes(m.invoice_id)"
                             @change="toggleTransferInvoice(m.invoice_id)" />
                      <span class="text-slate-600 truncate flex-1 min-w-24">{{ m.invoice.seller_name || m.invoice.invoice_number || m.invoice.id }}</span>
                      <span class="text-slate-400 shrink-0 tabular-nums">{{ m.invoice.travel_date || m.invoice.date }}</span>
                      <span class="font-semibold text-slate-800 shrink-0 tabular-nums">¥{{ fmt(m.invoice.amount) }}</span>
                    </label>
                  </template>
                </div>

                <!-- 手动挑选确认栏 -->
                <div v-if="manualPicking" class="mt-2 rounded-lg bg-white px-3 py-2 flex flex-wrap items-center gap-x-3 gap-y-1.5">
                  <span class="text-xs text-slate-500">
                    已选 {{ selectedTransferMatches.length }} 张 · 合计 <span class="font-semibold text-slate-800 tabular-nums">¥{{ fmt(selectedTransferTotal) }}</span>
                  </span>
                  <span v-if="transferCoverState === 'covered'" class="text-[11px] font-medium text-emerald-600">
                    已覆盖超额 ¥{{ fmt(analysis.cityTransport.overAmount) }}，移交后本趟可达标
                  </span>
                  <span v-else class="text-[11px] font-medium text-amber-600">
                    未覆盖超额（差 ¥{{ fmt(Math.max(analysis.cityTransport.overAmount - selectedTransferTotal, 0)) }}），仍可生成移交报销单
                  </span>
                  <input v-model="transferName" class="input-sm !w-36 shrink-0" placeholder="承接人姓名（选填）" />
                  <AppButton variant="primary" size="sm" :disabled="!selectedTransferMatches.length" @click="generateTransferForm">
                    确认生成移交报销单
                  </AppButton>
                </div>

                <!-- 移交报销单预览与导出 -->
                <div v-if="transferPreviewing && transferPreviewHtml" class="mt-2">
                  <div class="flex flex-wrap items-center justify-between gap-2 mb-1.5">
                    <p class="flex items-center gap-1.5 text-xs font-semibold text-slate-600">
                      <Eye :size="13" class="text-primary-600" />
                      移交报销单预览（仅含已选 {{ selectedTransferMatches.length }} 张发票）
                    </p>
                    <button type="button" class="text-[11px] font-medium text-slate-400 hover:text-slate-600 transition-colors"
                            @click="transferPreviewing = false">收起预览</button>
                  </div>
                  <div class="rounded-xl border border-slate-200 overflow-hidden">
                    <iframe :srcdoc="transferPreviewHtml" class="w-full" style="min-height: 420px; border: none;" title="移交报销单预览" />
                  </div>
                  <div class="mt-2 flex flex-wrap items-center gap-2">
                    <ExportButton
                      :match-results="selectedTransferMatches"
                      :unmatched-invoice-ids="[]"
                      :unmatched-payment-ids="[]"
                      :form-info="transferFormInfo"
                      show-labels
                    />
                    <button v-if="analysis.cityTransport.over" type="button"
                            class="text-[11px] font-medium text-rose-600 hover:text-rose-700 transition-colors"
                            title="发票已移交给同行人后，从本趟移除以免自己重复报销"
                            @click="removeTransferredFromTrip">
                      已移交？从本趟移除这 {{ selectedTransferMatches.length }} 张发票
                    </button>
                  </div>
                </div>
              </div>

              <!-- 提示二：尚未开票 → 选出行程重新开票移交 -->
              <div v-if="analysis.cityTransport.suggestedRides.length" class="mt-3">
                <p class="flex items-center gap-1.5 text-xs font-semibold text-slate-600">
                  <Route :size="13" class="text-rose-500" />
                  可重新开票移交的行程
                  <span class="font-normal text-slate-400">（{{ analysis.cityTransport.suggestedRides.length }} 笔 · 合计 ¥{{ fmt(analysis.cityTransport.suggestedRidesTotal) }}）</span>
                </p>
                <p class="mt-0.5 text-[11px] text-slate-400">尚未单独开票时，把以下行程在打车平台重新开具一张发票后交由同行人报销（每笔标注所属的现有发票，需换开）。</p>
                <div class="mt-1.5 space-y-1">
                  <div v-for="(ride, i) in analysis.cityTransport.suggestedRides" :key="ride.invoiceId + '-' + i"
                       class="flex flex-wrap items-center gap-x-2 gap-y-0.5 rounded-lg bg-white px-3 py-1.5 text-xs">
                    <span class="text-slate-400 shrink-0 tabular-nums">{{ ride.dateTime }}</span>
                    <span class="text-slate-700 truncate flex-1 min-w-32">{{ ride.pickup }} → {{ ride.dropoff }}</span>
                    <span class="text-slate-400 shrink-0" :title="ride.invoiceNumber || ride.invoiceLabel">{{ rideRef(ride) }}</span>
                    <span class="font-semibold text-rose-700 shrink-0 tabular-nums">¥{{ fmt(ride.amount) }}</span>
                  </div>
                </div>
                <p v-if="analysis.cityTransport.suggestedRidesTotal < analysis.cityTransport.overAmount - 0.005"
                   class="mt-1 text-[11px] text-slate-400">
                  行程合计不足以覆盖超额，建议优先按发票分摊。
                </p>
              </div>
              <p v-else class="mt-3 text-[11px] text-slate-400">
                未导入打车行程明细，无法给出重新开票的行程建议。
              </p>
            </div>

            <!-- 住宿费超标 -->
            <div v-if="analysis.hotel?.over" class="p-4">
              <div class="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5">
                <h4 class="flex items-center gap-1.5 text-sm font-bold text-rose-800">
                  <BedDouble :size="15" class="text-rose-500" />
                  住宿费超标 ¥{{ fmt(analysis.hotel.overAmount) }}
                </h4>
                <p class="text-xs text-slate-500 tabular-nums">标准 ¥{{ fmt(analysis.hotel.dailyRate) }}/晚</p>
              </div>
              <div class="mt-3 space-y-2">
                <div v-for="item in analysis.hotel.items" :key="item.invoiceId" class="rounded-lg bg-white px-3 py-2">
                  <div class="flex flex-wrap items-center justify-between gap-x-2 gap-y-0.5">
                    <span class="text-xs font-medium text-slate-700 truncate flex-1 min-w-24">{{ item.label }}</span>
                    <span class="text-[11px] text-slate-400 shrink-0 tabular-nums">
                      {{ item.dateRange }} · {{ item.nights }} 晚<template v-if="item.estimated"><span class="text-amber-600">（晚数按行程估算）</span></template>
                    </span>
                  </div>
                  <div class="mt-1 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs tabular-nums">
                    <span class="text-slate-500">实际 <span class="font-semibold text-slate-700">¥{{ fmt(item.actual) }}</span></span>
                    <span class="text-slate-500">标准 <span class="font-semibold text-slate-700">¥{{ fmt(item.standard) }}</span></span>
                    <span class="font-semibold text-rose-700">超标 ¥{{ fmt(item.overAmount) }}</span>
                    <span class="text-rose-500">平均每晚超 ¥{{ fmt(item.perNightOver) }}</span>
                  </div>
                </div>
              </div>
              <p class="mt-2 text-xs font-semibold text-rose-700 tabular-nums">
                合计超标 ¥{{ fmt(analysis.hotel.overAmount) }}<template v-if="analysis.hotel.overNights > 0"> · 平均每晚超标 ¥{{ fmt(analysis.hotel.overAmount / analysis.hotel.overNights) }}</template>
              </p>
            </div>
          </div>
        </Transition>
      </div>

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
            <div v-for="row in invoiceRows" :key="row.m.invoice_id"
                 class="flex items-start gap-2 px-4 py-2.5 hover:bg-slate-50 cursor-pointer transition-colors"
                 @click="openDetail(row.m.invoice)">
              <span class="w-20 shrink-0 text-xs font-medium chip border !py-0.5 !px-1.5 whitespace-nowrap" :class="getCategoryBadgeClass(row.m.invoice.category)">
                {{ CATEGORY_LABELS[row.m.invoice.category] }}
              </span>
              <div class="flex-1 min-w-0">
                <div class="flex flex-wrap items-center gap-2 text-sm">
                  <span class="text-slate-500 truncate flex-1 min-w-24">{{ row.m.invoice.seller_name || row.m.invoice.invoice_number || row.m.invoice.id }}</span>
                  <span class="text-slate-400 shrink-0">{{ row.m.invoice.travel_date || row.m.invoice.date }}</span>
                  <span class="font-semibold text-slate-800 shrink-0 tabular-nums">¥{{ row.m.invoice.amount.toFixed(2) }}</span>
                  <span class="text-primary-600 text-xs shrink-0">详情</span>
                  <select :value="trip.id" @click.stop @change="handleMoveInvoice(row.m.invoice_id, ($event.target as HTMLSelectElement).value)"
                          class="input-sm !w-auto shrink-0 cursor-pointer">
                    <option v-for="t in otherTrips" :key="t.id" :value="t.id">出差 {{ t.destination || '未设置' }} {{ t.travelStart }}~{{ t.travelEnd }}</option>
                    <option value="">移到待调整</option>
                  </select>
                </div>
                <!-- 次要信息行：发票提取的时间事实 + 匹配支付的交易时间（用于区分交易） -->
                <div v-if="row.detail.total" class="mt-1 flex flex-wrap items-baseline gap-x-3 gap-y-0.5 text-xs leading-5">
                  <span v-for="(seg, i) in row.detail.segments" :key="i" class="inline-flex items-baseline gap-1 min-w-0 max-w-full">
                    <span v-if="seg.label" class="text-slate-400 shrink-0">{{ seg.label }}</span>
                    <span class="text-slate-500 truncate tabular-nums" :title="seg.value">{{ seg.value }}</span>
                  </span>
                </div>
              </div>
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
import { computed, ref, nextTick, onMounted } from 'vue'
import { watchDebounced } from '@vueuse/core'
import { invoke } from '@tauri-apps/api/core'
import { MapPin, CalendarDays, Receipt, ChevronDown, Eye, AlertTriangle, CarTaxiFront, BedDouble, Route } from 'lucide-vue-next'
import AppButton from './ui/AppButton.vue'
import DonutChart from './ui/DonutChart.vue'
import ReimbursementForm from './ReimbursementForm.vue'
import ExportButton from './ExportButton.vue'
import InvoiceDetailModal from './InvoiceDetailModal.vue'
import { useMatchStore } from '../stores/match'
import { useInvoiceStore } from '../stores/invoice'
import { toast } from '../composables/toast'
import { useProfile } from '../composables/profile'
import { analyzeTripOverage } from '../utils/overage'
import { analyzeStayDays } from '../utils/stay'
import type { Invoice, MatchResult, Trip, ReimbursementForm as ReimbursementFormResult } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import { getCategoryBadgeClass } from '../utils/category'

const { profile } = useProfile()

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

const showOverage = ref(false)
const overagePanel = ref<HTMLElement | null>(null)

const tripTotal = computed(() => props.trip.matches.reduce((s, m) => s + m.invoice.amount, 0))

// ── 发票明细行的次要信息：时间事实 + 匹配支付交易时间 ─────────────
interface DetailSegment { label: string; value: string }
interface InvoiceDetail { segments: DetailSegment[]; total: boolean }

const SOURCE_LABELS: Record<string, string> = { Wechat: '微信', Alipay: '支付宝' }

// 行程时间补全年份：行程单常缺年份（如 "06-15 10:30"），用发票开票日期的年份补齐
function normalizeRideTime(raw: string, invoiceDate: string): string {
  const t = raw.trim()
  if (!t || /^\d{4}-/.test(t)) return t
  const year = invoiceDate ? invoiceDate.slice(0, 5) : ''
  return year && /^\d{2}-\d{2}/.test(t) ? year + t : t
}

function buildInvoiceDetail(m: MatchResult): InvoiceDetail {
  const inv = m.invoice
  const segments: DetailSegment[] = []

  // 发票号尾号：区分同一卖方的多张发票
  const tailSegment: DetailSegment | null = inv.invoice_number
    ? { label: '尾号', value: inv.invoice_number.slice(-6) }
    : null

  // 发票提取的时间/行程事实（按类别）
  if ((inv.category === 'Train' || inv.category === 'Flight') && inv.travel_date) {
    segments.push({ label: '出行', value: inv.travel_time ? `${inv.travel_date} ${inv.travel_time}` : inv.travel_date })
  }
  if (inv.category === 'TicketChange' && inv.date) {
    segments.push({ label: '开票', value: inv.date })
  }
  if (inv.category === 'Hotel' && inv.hotel_detail) {
    const hd = inv.hotel_detail
    if (hd.check_in) segments.push({ label: '入住', value: hd.check_in })
    if (hd.check_out) segments.push({ label: '离店', value: hd.check_out })
    if (hd.nights > 0) segments.push({ label: '', value: `${hd.nights} 晚` })
    if (hd.nightly_rate > 0) segments.push({ label: '', value: `¥${hd.nightly_rate.toFixed(2)}/晚` })
  }
  if (inv.category === 'Toll' && inv.toll_travel_time) {
    segments.push({ label: '通行', value: inv.toll_travel_time.replace('T', ' ') })
  }
  if (inv.category === 'Train' || inv.category === 'Flight') {
    const route = [inv.departure_city, inv.arrival_city].filter(Boolean).join(' → ')
    if (route) segments.push({ label: '行程', value: route })
  }

  // 网约车：只显示行程数量与首末行程时间（行程与支付明细见详情弹窗）
  if (inv.category === 'CityTransport') {
    const its = inv.itineraries
    if (its.length === 1) {
      segments.push({ label: '行程', value: ['1 笔', normalizeRideTime(its[0].date_time, inv.date)].filter(Boolean).join(' ') })
    } else if (its.length > 1) {
      const first = normalizeRideTime(its[0].date_time, inv.date)
      const last = normalizeRideTime(its[its.length - 1].date_time, inv.date)
      const sameDay = first.length >= 16 && last.length >= 16 && first.slice(0, 10) === last.slice(0, 10)
      const range = sameDay ? `${first} ~ ${last.slice(11)}` : [first, last].filter(Boolean).join(' ~ ')
      segments.push({ label: '行程', value: `${its.length} 笔 ${range}`.trim() })
    }
    if (tailSegment) segments.push(tailSegment)
    return { segments, total: segments.length > 0 }
  }

  // 匹配支付的交易时间（区分同票面多笔交易的关键信息）
  for (const p of m.payments) {
    segments.push({ label: SOURCE_LABELS[p.source] ?? '交易', value: p.transaction_time })
  }

  if (inv.item_name) segments.push({ label: '商品', value: inv.item_name })
  if (tailSegment) segments.push(tailSegment)

  return { segments, total: segments.length > 0 }
}

const invoiceRows = computed(() =>
  props.trip.matches.map(m => ({ m, detail: buildInvoiceDetail(m) })),
)

// 报销表单：按最新报销标准实时计算（后端 build_reimbursement_form），圆环与超标分析同源
const formResult = ref<ReimbursementFormResult | null>(null)
// 首次计算是否已结束（成功或失败），未成功过则回退显示发票原始合计
const firstSettled = ref(false)
// 请求序号：并发/竞态时只采用最后一次结果
let requestSeq = 0

const displayTotal = computed(() => {
  const total = formResult.value?.total_amount
  if (total !== undefined) return '¥' + total.toFixed(2)
  return firstSettled.value ? '¥' + tripTotal.value.toFixed(2) : '…'
})

// 超标分析：与 formResult 同源更新，trip 自身变化由响应式追踪
const analysis = computed(() => (formResult.value ? analyzeTripOverage(props.trip, formResult.value) : null))

// 住宿天数与行程天数核对：null 表示无需提示（无住宿发票/日期不完整/正好对应）
const stayCheck = computed(() => analyzeStayDays(props.trip))

// 分类别目的圆环数据（各自的标准使用率与超标状态）
const ctRing = computed(() => {
  const ct = analysis.value?.cityTransport
  if (!ct || ct.usageRate === null) return null
  return { rate: ct.usageRate, over: ct.over, title: ringTitle('市内交通', ct.over, ct.usageRate) }
})
const hotelRing = computed(() => {
  const h = analysis.value?.hotel
  if (!h || h.usageRate === null) return null
  return { rate: h.usageRate, over: h.over, title: ringTitle('住宿', h.over, h.usageRate) }
})

function ringTitle(name: string, over: boolean, rate: number): string {
  const base = `${name}费用为报销标准的 ${Math.round(rate)}%`
  return over ? `${base}，已超标，点击查看超标分析` : `${base}，未超标`
}

function toggleOverage() {
  showOverage.value = !showOverage.value
  if (showOverage.value) {
    nextTick(() => overagePanel.value?.scrollIntoView({ behavior: 'smooth', block: 'nearest' }))
  }
}

const fmt = (n: number) => n.toFixed(2)
// 标准值（元/天）等非金额数：整数不带小数位
const fmtStd = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(2))
// 行程所属发票的展示引用：有发票号给尾号，否则退回卖方/票号
function rideRef(ride: { invoiceNumber?: string; invoiceLabel: string }): string {
  return ride.invoiceNumber ? `所属发票尾号 ${ride.invoiceNumber.slice(-6)}` : `所属 ${ride.invoiceLabel}`
}

// ── 移交发票手动挑选与移交报销单生成 ─────────────────────────
const manualPicking = ref(false)
const selectedTransferIds = ref<string[]>([])
const transferName = ref('')
const transferPreviewing = ref(false)
const transferPreviewHtml = ref<string | null>(null)

const transferCandidates = computed(() =>
  props.trip.matches.filter(m => m.invoice.category === 'CityTransport' || m.invoice.category === 'Toll'),
)
const selectedTransferMatches = computed(() =>
  transferCandidates.value.filter(m => selectedTransferIds.value.includes(m.invoice_id)),
)
const selectedTransferTotal = computed(() => selectedTransferMatches.value.reduce((s, m) => s + m.invoice.amount, 0))
const transferCoverState = computed<'none' | 'covered' | 'short'>(() => {
  const ct = analysis.value?.cityTransport
  if (!ct?.over) return 'none'
  return selectedTransferTotal.value >= ct.overAmount - 0.005 ? 'covered' : 'short'
})

// 移交报销单的表单信息：姓名为承接人（选填），行程信息沿用本趟
const transferFormInfo = computed(() => ({
  name: transferName.value.trim(),
  department: '',
  destination: props.trip.destination,
  travelStart: props.trip.travelStart,
  travelEnd: props.trip.travelEnd,
  companions: profile.value.companions,
  hotelLevel: props.trip.hotelLevel,
}))

function enterManualPick() {
  // 预选自动建议的最小移交组合，用户可增删
  selectedTransferIds.value = analysis.value?.cityTransport?.suggestedInvoices.map(s => s.invoiceId) ?? []
  transferPreviewing.value = false
  transferPreviewHtml.value = null
  manualPicking.value = true
}

function cancelManualPick() {
  manualPicking.value = false
  selectedTransferIds.value = []
  transferName.value = ''
  transferPreviewing.value = false
  transferPreviewHtml.value = null
}

function toggleTransferInvoice(invoiceId: string) {
  const idx = selectedTransferIds.value.indexOf(invoiceId)
  if (idx >= 0) selectedTransferIds.value.splice(idx, 1)
  else selectedTransferIds.value.push(invoiceId)
}

async function generateTransferForm() {
  if (!selectedTransferMatches.value.length) return
  try {
    const html = await matchStore.renderReimbursementHtml(transferFormInfo.value, selectedTransferMatches.value)
    transferPreviewHtml.value = html
    transferPreviewing.value = true
  } catch (e) {
    console.error('生成移交报销单失败:', e)
    toast('生成移交报销单失败: ' + e, 'error')
  }
}

function removeTransferredFromTrip() {
  const ids = selectedTransferMatches.value.map(m => m.invoice_id)
  for (const id of ids) emit('move', id, null)
  toast(`已将 ${ids.length} 张移交发票移到待调整区`, 'success')
  cancelManualPick()
}

async function refreshReimbursable() {
  const seq = ++requestSeq
  if (!props.trip.matches.length) {
    formResult.value = null
    firstSettled.value = true
    return
  }
  try {
    const form = await invoke<ReimbursementFormResult>('preview_reimbursement_form', {
      matchResults: props.trip.matches,
      name: profile.value.name,
      department: profile.value.department,
      destination: props.trip.destination,
      travelStart: props.trip.travelStart,
      travelEnd: props.trip.travelEnd,
      companions: profile.value.companions,
      hotelLevel: props.trip.hotelLevel,
    })
    if (seq !== requestSeq) return // 丢弃过期结果
    formResult.value = form
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
  name: profile.value.name,
  department: profile.value.department,
  destination: props.trip.destination,
  travelStart: props.trip.travelStart,
  travelEnd: props.trip.travelEnd,
  companions: profile.value.companions,
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
