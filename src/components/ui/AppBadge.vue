<template>
  <span class="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium"
        :class="toneClass">
    <span class="w-1.5 h-1.5 rounded-full" :class="dotClass" />
    <slot />
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'

export type BadgeTone = 'success' | 'warning' | 'danger' | 'neutral' | 'info'

const props = withDefaults(defineProps<{ tone?: BadgeTone }>(), { tone: 'neutral' })

const toneClass = computed(() => ({
  success: 'bg-emerald-50 text-emerald-700',
  warning: 'bg-amber-50 text-amber-700',
  danger: 'bg-red-50 text-red-700',
  neutral: 'bg-gray-100 text-gray-600',
  info: 'bg-primary-50 text-primary-700',
}[props.tone]))

const dotClass = computed(() => ({
  success: 'bg-emerald-500',
  warning: 'bg-amber-500',
  danger: 'bg-red-500',
  neutral: 'bg-gray-400',
  info: 'bg-primary-500',
}[props.tone]))
</script>
