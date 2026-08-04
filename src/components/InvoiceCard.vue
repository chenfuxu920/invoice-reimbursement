<template>
  <div class="card card-hover p-4 cursor-pointer animate-fade-in-up" @click="expanded = !expanded">
    <div class="flex justify-between items-start gap-3">
      <!-- 左侧：类别图标 + 金额/销售方 -->
      <div class="flex items-start gap-3 flex-1 min-w-0">
        <span class="w-11 h-11 rounded-xl flex items-center justify-center shrink-0 shadow-card"
              :class="getCategoryIconWrap(invoice.category)">
          <AppIcon :name="getCategoryIcon(invoice.category)" :size="19" />
        </span>
        <div class="min-w-0">
          <div class="flex items-center gap-2 flex-wrap">
            <span class="chip border" :class="getCategoryBadgeClass(invoice.category)">{{ getCategoryLabel(invoice.category) }}</span>
            <span v-if="invoice.source.type === 'Manual'" class="chip bg-slate-100 text-slate-500 border-slate-200/70">手工补录</span>
          </div>
          <p class="font-display text-xl font-extrabold text-slate-900 mt-1.5 tabular-nums">¥{{ invoice.amount.toFixed(2) }}</p>
          <p class="text-sm text-slate-500 truncate cursor-pointer hover:text-primary-600 transition-colors"
             :title="invoice.seller_name || '未知销售方'" @click.stop="$emit('view-detail', invoice)">
            {{ invoice.seller_name || '未知销售方' }}
            <span class="text-primary-600 text-xs">查看详情 →</span>
          </p>
        </div>
      </div>
      <!-- 右侧操作 -->
      <div class="flex items-center gap-1 shrink-0">
        <button @click.stop="$emit('remove', invoice.id)"
                class="w-7 h-7 rounded-lg flex items-center justify-center text-slate-300 hover:text-rose-500 hover:bg-rose-50 transition-all"
                :title="'删除'" :aria-label="'删除'">
          <X :size="14" />
        </button>
        <span class="text-slate-300 transition-transform duration-300" :class="{ 'rotate-180': expanded }">
          <ChevronDown :size="16" />
        </span>
      </div>
    </div>

    <div class="mt-3 flex items-center gap-4 text-xs text-slate-400 border-t border-slate-100 pt-2.5">
      <span class="flex items-center gap-1 min-w-0">
        <Hash :size="11" class="shrink-0" />
        <span class="truncate">发票号: {{ invoice.invoice_number || '无' }}</span>
      </span>
      <span class="flex items-center gap-1 shrink-0">
        <CalendarDays :size="11" />
        {{ invoice.date }}
      </span>
    </div>

    <!-- 内联展开摘要 -->
    <div v-if="expanded" class="mt-3 pt-3 border-t border-slate-100 space-y-2 text-sm animate-fade-in">
      <div class="flex flex-wrap gap-x-6 gap-y-1 text-slate-600">
        <span class="text-xs">商品/服务: <b class="font-medium">{{ invoice.item_name || '无' }}</b></span>
        <span class="text-xs">来源文件: <b class="font-medium">{{ sourceFileName }}</b></span>
      </div>
      <div v-if="invoice.itineraries?.length">
        <p class="text-xs font-medium text-slate-500 mb-1.5">行程明细 ({{ invoice.itineraries.length }})</p>
        <div class="space-y-1.5">
          <div v-for="(it, i) in invoice.itineraries" :key="i"
               class="flex items-center gap-2 rounded-xl px-3 py-2 text-xs"
               :class="it.incomplete_fields?.length ? 'bg-amber-50 border border-amber-200/70' : 'bg-primary-50/70 border border-primary-100'">
            <span v-if="it.incomplete_fields?.length" class="text-amber-500 cursor-help shrink-0"
                  :title="'缺失: ' + it.incomplete_fields.map(f => fieldLabel(f)).join(', ')"><AlertTriangle :size="13" /></span>
            <span class="flex-1 min-w-0 text-slate-600 truncate">{{ it.date_time }} | {{ it.provider }} | {{ it.pickup }} → {{ it.dropoff }}</span>
            <span class="font-semibold text-slate-700 tabular-nums shrink-0">¥{{ it.amount.toFixed(2) }}</span>
          </div>
        </div>
      </div>
      <p v-else class="text-xs text-slate-400">无行程明细</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { X, ChevronDown, Hash, CalendarDays, AlertTriangle } from 'lucide-vue-next'
import type { Invoice, InvoiceCategory } from '../types'
import AppIcon from './ui/AppIcon.vue'
import { getCategoryLabel, getCategoryBadgeClass, getCategoryIcon } from '../utils/category'

const props = defineProps<{ invoice: Invoice }>()
defineEmits<{
  (e: 'remove', id: string): void
  (e: 'view-detail', invoice: Invoice): void
}>()

const expanded = ref(false)

const ICON_WRAPS: Record<InvoiceCategory, string> = {
  Train: 'bg-emerald-100 text-emerald-600',
  Flight: 'bg-primary-100 text-primary-600',
  Insurance: 'bg-cyan-100 text-cyan-600',
  TicketChange: 'bg-amber-100 text-amber-600',
  CityTransport: 'bg-violet-100 text-violet-600',
  Hotel: 'bg-yellow-100 text-yellow-600',
  Meal: 'bg-rose-100 text-rose-600',
  Toll: 'bg-indigo-100 text-indigo-600',
  Other: 'bg-slate-100 text-slate-600',
}

function getCategoryIconWrap(category: InvoiceCategory) {
  return ICON_WRAPS[category] || ICON_WRAPS.Other
}

const sourceFileName = computed(() => {
  const p = props.invoice.source.path
  if (!p) return '手动添加'
  const parts = p.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || p
})

function fieldLabel(f: string): string {
  const map: Record<string, string> = {
    date_time: '时间', provider: '服务商', pickup: '起点', dropoff: '终点', amount: '金额',
  }
  return map[f] || f
}
</script>
