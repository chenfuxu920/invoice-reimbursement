<template>
  <div role="group" aria-label="按发票类别筛选" class="flex flex-wrap items-center gap-1.5">
    <button type="button" :class="allChipClass" :aria-pressed="modelValue === null"
            @click="emit('update:modelValue', null)">
      全部
      <span class="count-pill inline-flex items-center justify-center px-1.5 min-w-5 h-4 rounded-full text-[10px] leading-none font-semibold tabular-nums" :class="modelValue === null ? 'bg-white/25 text-white' : 'bg-slate-100 text-slate-500'">{{ total }}</span>
    </button>
    <button v-for="cat in visibleCategories" :key="cat" type="button"
            :class="chipClass(cat)" :aria-pressed="modelValue === cat"
            @click="toggle(cat)">
      <AppIcon :name="getCategoryIcon(cat)" :size="13" />
      {{ getCategoryLabel(cat) }}
      <span class="count-pill inline-flex items-center justify-center px-1.5 min-w-5 h-4 rounded-full text-[10px] leading-none font-semibold tabular-nums" :class="modelValue === cat ? 'bg-white/70 text-inherit' : 'bg-slate-100 text-slate-500'">{{ counts[cat] }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import AppIcon from './AppIcon.vue'
import type { InvoiceCategory } from '../../types'
import { CATEGORY_LABELS } from '../../types/invoice'
import { getCategoryIcon, getCategoryLabel, getCategoryBadgeClass } from '../../utils/category'

const props = defineProps<{
  /// 各类别数量，仅数量 > 0 的类别会显示
  counts: Partial<Record<InvoiceCategory, number>>
  /// 全部类别的总数
  total: number
  modelValue: InvoiceCategory | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: InvoiceCategory | null): void
}>()

const CHIP_BASE = 'inline-flex items-center gap-1.5 rounded-full px-2.5 py-1.5 text-xs border transition-all duration-200 cursor-pointer select-none active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-400/50'
const CHIP_IDLE = 'bg-white text-slate-500 border-slate-200 hover:border-primary-300 hover:text-primary-700'

const visibleCategories = computed(() =>
  (Object.keys(CATEGORY_LABELS) as InvoiceCategory[]).filter(c => (props.counts[c] ?? 0) > 0)
)

const allChipClass = computed(() => [
  CHIP_BASE,
  'font-medium',
  props.modelValue === null
    ? 'bg-primary-600 text-white border-primary-600 shadow-glow-sm font-semibold'
    : CHIP_IDLE,
])

function chipClass(cat: InvoiceCategory) {
  return [
    CHIP_BASE,
    props.modelValue === cat
      ? [getCategoryBadgeClass(cat), 'border-current/30 shadow-sm font-semibold']
      : CHIP_IDLE,
  ]
}

function toggle(cat: InvoiceCategory) {
  emit('update:modelValue', props.modelValue === cat ? null : cat)
}
</script>
