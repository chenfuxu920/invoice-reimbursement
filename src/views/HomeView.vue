<template>
  <div class="max-w-5xl mx-auto px-5 py-6 space-y-6">
    <!-- 英雄区 -->
    <section class="relative overflow-hidden rounded-3xl border border-white/70 bg-white/70 shadow-card p-8 md:p-10">
      <!-- 光晕装饰 -->
      <div class="absolute -top-24 -right-16 w-80 h-80 rounded-full bg-primary-400/20 animate-drift pointer-events-none" />
      <div class="absolute -bottom-28 -left-20 w-80 h-80 rounded-full bg-accent-400/15 animate-drift pointer-events-none" style="animation-delay: -8s" />
      <div class="absolute top-6 right-10 w-24 h-24 rounded-2xl bg-gradient-to-br from-primary-500/10 to-accent-500/10 border border-primary-200/40 rotate-12 animate-float hidden md:block pointer-events-none" />

      <div class="relative">
        <span class="chip bg-gradient-to-r from-primary-600 to-accent-500 text-white border-transparent shadow-glow-sm mb-5 animate-fade-in-up">
          <Sparkles :size="13" /> 智能报销工作台
        </span>
        <h1 class="font-display text-3xl md:text-4xl font-extrabold text-slate-900 leading-tight animate-fade-in-up" style="animation-delay: 60ms">
          发票报销，<span class="text-gradient">三步搞定</span>
        </h1>
        <p class="mt-3 text-slate-500 text-sm md:text-base max-w-lg leading-relaxed animate-fade-in-up" style="animation-delay: 120ms">
          拖入发票与账单，自动识别、自动匹配、一键打包成报销材料。
          <span class="text-slate-700 font-medium">不用学，跟着流程走就行。</span>
        </p>

        <div class="mt-7 flex flex-wrap items-center gap-4 animate-fade-in-up" style="animation-delay: 180ms">
          <button class="btn-primary-glow px-7 py-3 text-base" @click="$router.push(primaryCta.to)">
            {{ primaryCta.label }}
            <ArrowRight :size="18" />
          </button>
          <p class="text-sm text-slate-500">{{ primaryCta.hint }}</p>
        </div>

        <!-- OCR 状态小芯片（不再独占大卡片） -->
        <div class="mt-6 flex flex-wrap items-center gap-2 animate-fade-in-up" style="animation-delay: 240ms">
          <span class="chip border shadow-card" :class="ocrOnline ? 'bg-emerald-50 text-emerald-700 border-emerald-200/70' : 'bg-rose-50 text-rose-700 border-rose-200/70'">
            <span class="w-1.5 h-1.5 rounded-full" :class="ocrOnline ? 'bg-emerald-500 animate-pulse-soft' : 'bg-rose-500'" />
            OCR 识别 {{ ocrOnline ? '在线' : '离线' }}
          </span>

          <template v-if="!ocrOnline">
            <button v-if="!downloadingModels" class="chip bg-white text-primary-700 border border-primary-200 shadow-card hover:bg-primary-50 hover:shadow-glow-sm transition-all cursor-pointer"
                    @click="downloadModels">
              <Download :size="13" /> 下载 OCR 模型（约 20MB）
            </button>
            <span v-else class="chip bg-white text-primary-700 border border-primary-200 shadow-card">
              <Loader2 :size="13" class="animate-spin" />
              下载中 {{ downloadProgress.file }} ({{ downloadProgress.index + 1 }}/{{ downloadProgress.total }})
            </span>
            <button class="text-xs text-slate-400 hover:text-primary-600 transition-colors" title="下载地址设置" @click="showConfig = !showConfig">
              <Settings2 :size="14" />
            </button>
          </template>
          <span v-else class="chip bg-white text-slate-500 border border-slate-200 shadow-card">
            <CheckCircle2 :size="13" class="text-emerald-500" /> 模型已就绪
          </span>

          <!-- 下载地址设置（内联展开） -->
          <div v-if="showConfig" class="flex items-center gap-2 animate-scale-in">
            <input v-model="modelBaseUrl" class="input !w-96 !py-1.5 !text-xs" placeholder="https://github.com/.../releases/download/ocr-models-v1" />
            <AppButton size="sm" @click="saveConfig">保存</AppButton>
          </div>
        </div>
      </div>
    </section>

    <!-- 三张流程状态卡 -->
    <section class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <div v-for="(card, i) in flowCards" :key="card.to"
           class="card card-hover p-5 cursor-pointer animate-fade-in-up"
           :class="{ 'opacity-80': card.locked && !card.done }"
           :style="{ animationDelay: `${i * 90}ms` }"
           @click="$router.push(card.to)">
        <div class="flex items-start justify-between mb-4">
          <span class="w-11 h-11 rounded-2xl flex items-center justify-center text-white shadow-glow-sm"
                :class="card.gradient">
            <component :is="card.icon" :size="20" />
          </span>
          <AppBadge :tone="card.badge.tone" :dot="false">{{ card.badge.label }}</AppBadge>
        </div>
        <h3 class="font-display text-base font-bold text-slate-800">{{ card.title }}</h3>
        <p class="text-xs text-slate-400 mt-1 mb-4">{{ card.desc }}</p>
        <div class="flex items-end justify-between">
          <div class="space-y-0.5">
            <p v-for="s in card.stats" :key="s.label" class="text-sm text-slate-500">
              <span class="font-bold text-lg text-slate-800 tabular-nums mr-1">{{ s.value }}</span>{{ s.label }}
            </p>
          </div>
          <span class="inline-flex items-center gap-1 text-sm font-medium"
                :class="card.done ? 'text-emerald-600' : card.locked ? 'text-slate-300' : 'text-primary-600'">
            {{ card.action }}
            <ArrowRight :size="14" />
          </span>
        </div>
      </div>
    </section>

    <!-- 快捷入口 -->
    <section class="grid grid-cols-2 md:grid-cols-4 gap-3">
      <button v-for="(q, i) in quickActions" :key="q.label" @click="$router.push(q.to)"
              class="card card-hover flex items-center gap-3 px-4 py-3.5 text-left animate-fade-in-up group"
              :style="{ animationDelay: `${i * 70}ms` }">
        <span class="w-9 h-9 rounded-xl flex items-center justify-center shrink-0 transition-transform duration-200 group-hover:scale-110"
              :class="q.wrap">
          <component :is="q.icon" :size="17" />
        </span>
        <span class="min-w-0">
          <span class="block text-sm font-medium text-slate-700 truncate">{{ q.label }}</span>
          <span class="block text-xs text-slate-400 truncate">{{ q.hint }}</span>
        </span>
      </button>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import {
  ArrowRight, Sparkles, Download, Loader2, Settings2, CheckCircle2,
  Upload, Link2, Package, Plus, Wand2, FolderOpen, ClipboardList,
} from 'lucide-vue-next'
import AppButton from '../components/ui/AppButton.vue'
import AppBadge from '../components/ui/AppBadge.vue'
import { useInvoiceStore } from '../stores/invoice'
import { usePaymentStore } from '../stores/payment'
import { useMatchStore } from '../stores/match'
import { useOcrStatus } from '../composables/ocr'
import { useCountUp } from '../composables/useCountUp'
import { toast } from '../composables/toast'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { BadgeTone } from '../components/ui/AppBadge.vue'

const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()
const { ocrOnline, refresh } = useOcrStatus()

const downloadingModels = ref(false)
const showConfig = ref(false)
const modelBaseUrl = ref('')
const downloadProgress = ref({ file: '', index: 0, total: 0 })

onMounted(async () => {
  try {
    const config = await invoke<{ model_base_url: string }>('get_ocr_model_config')
    modelBaseUrl.value = config.model_base_url
  } catch { /* 使用默认值 */ }
  await listen<{ file: string; index: number; total: number }>('ocr-download-progress', (e) => {
    downloadProgress.value = e.payload
  })
})

async function downloadModels() {
  downloadingModels.value = true
  try {
    await invoke('download_ocr_models')
    await refresh()
    downloadingModels.value = false
    toast(ocrOnline.value
      ? 'OCR 模型下载完成，识别服务已就绪，可直接使用，无需重启。'
      : 'OCR 模型下载完成，但引擎未就绪，请重启应用后使用。', ocrOnline.value ? 'success' : 'error')
  } catch (e) {
    downloadingModels.value = false
    toast(`下载失败: ${e}`, 'error')
  }
}

async function saveConfig() {
  try {
    await invoke('set_ocr_model_config', { modelBaseUrl: modelBaseUrl.value })
    showConfig.value = false
    toast('下载地址已保存', 'success')
  } catch (e) {
    toast(`保存失败: ${e}`, 'error')
  }
}

/* ── 数据状态 ── */
const hasInvoices = computed(() => invoiceStore.invoices.length > 0)
const hasPayments = computed(() => paymentStore.payments.length > 0)
const hasMatches = computed(() => matchStore.matches.length > 0)

const primaryCta = computed(() => {
  if (!hasInvoices.value) return { to: '/import', label: '开始收集票据', hint: '拖入发票与微信 / 支付宝账单即可自动识别' }
  if (!hasPayments.value) return { to: '/import', label: '导入支付账单', hint: '已有发票，还需导入账单才能匹配' }
  if (!hasMatches.value) return { to: '/match', label: '去核对匹配', hint: `待匹配发票 ${invoiceStore.invoices.length} 张、账单 ${paymentStore.payments.length} 条` }
  if (matchStore.trips.length === 0) return { to: '/export', label: '去打包导出', hint: '匹配完成，确认分趟后即可生成报销材料' }
  return { to: '/export', label: '查看报销档案', hint: `已分趟 ${matchStore.trips.length} 趟，可随时导出` }
})

const invoiceCount = useCountUp(() => invoiceStore.invoices.length)
const paymentCount = useCountUp(() => paymentStore.payments.length)
const matchCount = useCountUp(() => matchStore.matches.length)
const tripCount = useCountUp(() => matchStore.trips.length)
const unmatchedCount = useCountUp(() => matchStore.unmatchedInvoices.length + matchStore.unmatchedPayments.length)

const flowCards = computed(() => {
  const invoicesDone = hasInvoices.value && hasPayments.value
  const matchDone = hasMatches.value
  const tripDone = matchStore.trips.length > 0
  return [
    {
      to: '/import',
      title: '① 收集票据',
      desc: '发票与账单拖进来，自动分类识别',
      icon: Upload,
      gradient: 'bg-gradient-to-br from-primary-500 to-accent-500',
      badge: invoicesDone
        ? { label: '已完成', tone: 'success' as BadgeTone }
        : (hasInvoices.value || hasPayments.value)
          ? { label: '进行中', tone: 'info' as BadgeTone }
          : { label: '未开始', tone: 'neutral' as BadgeTone },
      stats: [
        { label: '张发票', value: String(invoiceCount.value) },
        { label: '条账单', value: String(paymentCount.value) },
      ],
      action: invoicesDone ? '查看' : '去收集',
      done: invoicesDone,
      locked: false,
    },
    {
      to: '/match',
      title: '② 核对匹配',
      desc: '发票与支付记录自动配对，人工可微调',
      icon: Link2,
      gradient: 'bg-gradient-to-br from-accent-500 to-flare-500',
      badge: matchDone
        ? { label: '已完成', tone: 'success' as BadgeTone }
        : (hasInvoices.value && hasPayments.value)
          ? { label: '进行中', tone: 'warning' as BadgeTone }
          : { label: '待收集', tone: 'neutral' as BadgeTone },
      stats: [
        { label: '个匹配', value: String(matchCount.value) },
        { label: '项待处理', value: String(unmatchedCount.value) },
      ],
      action: matchDone ? '查看' : '去匹配',
      done: matchDone,
      locked: !(hasInvoices.value && hasPayments.value),
    },
    {
      to: '/export',
      title: '③ 打包导出',
      desc: '按出差分趟，一键生成报销材料',
      icon: Package,
      gradient: 'bg-gradient-to-br from-emerald-500 to-teal-500',
      badge: tripDone
        ? { label: '已完成', tone: 'success' as BadgeTone }
        : matchDone
          ? { label: '待整理', tone: 'warning' as BadgeTone }
          : { label: '待匹配', tone: 'neutral' as BadgeTone },
      stats: [
        { label: '趟出差', value: String(tripCount.value) },
        { label: '项待调整', value: String(matchStore.unassigned.length) },
      ],
      action: tripDone ? '查看' : '去导出',
      done: tripDone,
      locked: !hasMatches.value,
    },
  ]
})

const quickActions = [
  { to: '/import', label: '手动添加空发票', hint: '纸质票据补录', icon: Plus, wrap: 'bg-primary-50 text-primary-600' },
  { to: '/import', label: '全局导入', hint: '选择文件夹批量导入', icon: FolderOpen, wrap: 'bg-violet-50 text-violet-600' },
  { to: '/match', label: '手动匹配', hint: '调整配对关系', icon: Wand2, wrap: 'bg-amber-50 text-amber-600' },
  { to: '/export', label: '一键导出全部', hint: '每趟单独成文件', icon: ClipboardList, wrap: 'bg-emerald-50 text-emerald-600' },
]
</script>
