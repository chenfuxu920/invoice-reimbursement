<template>
  <span class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium border"
        :class="toneClass">
    <span v-if="showDot" class="w-1.5 h-1.5 rounded-full" :class="dotClass" />
    <slot />
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'

export type BadgeTone = 'success' | 'warning' | 'danger' | 'neutral' | 'info' | 'violet'

const props = withDefaults(defineProps<{ tone?: BadgeTone; dot?: boolean }>(), { tone: 'neutral', dot: true })

const showDot = computed(() => props.dot)

const toneClass = computed(() => ({
  success: 'bg-emerald-50 text-emerald-700 border-emerald-200/70',
  warning: 'bg-amber-50 text-amber-700 border-amber-200/70',
  danger: 'bg-rose-50 text-rose-700 border-rose-200/70',
  neutral: 'bg-slate-100 text-slate-600 border-slate-200/70',
  info: 'bg-primary-50 text-primary-700 border-primary-200/70',
  violet: 'bg-violet-50 text-violet-700 border-violet-200/70',
}[props.tone]))

const dotClass = computed(() => ({
  success: 'bg-emerald-500',
  warning: 'bg-amber-500',
  danger: 'bg-rose-500',
  neutral: 'bg-slate-400',
  info: 'bg-primary-500',
  violet: 'bg-violet-500',
}[props.tone]))
</script>
