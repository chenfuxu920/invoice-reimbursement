<template>
  <button :disabled="disabled || loading" class="inline-flex items-center justify-center gap-1.5 rounded-xl text-sm font-medium transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed disabled:shadow-none"
          :class="[sizeClass, variantClass]" :title="title" :aria-label="ariaLabel">
    <Loader2 v-if="loading" class="animate-spin shrink-0" :size="14" />
    <slot />
  </button>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Loader2 } from 'lucide-vue-next'

const props = withDefaults(defineProps<{
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger' | 'soft'
  size?: 'sm' | 'md' | 'lg' | 'icon'
  disabled?: boolean
  loading?: boolean
  title?: string
  ariaLabel?: string
}>(), { variant: 'secondary', size: 'md' })

const sizeClass = computed(() => {
  if (props.size === 'icon') return 'w-9 h-9 p-0 justify-center'
  if (props.size === 'lg') return 'px-5 py-2.5 text-base'
  return props.size === 'sm' ? 'px-3 py-1.5 text-xs' : 'px-4 py-2'
})
const variantClass = computed(() => ({
  primary: 'btn-primary-glow',
  secondary: 'bg-white text-slate-700 border border-slate-300 hover:border-primary-400 hover:text-primary-700 hover:shadow-card',
  soft: 'bg-primary-50 text-primary-700 hover:bg-primary-100',
  ghost: 'text-slate-600 hover:bg-slate-100 hover:text-slate-900',
  danger: 'bg-gradient-to-r from-rose-500 to-red-500 text-white shadow-glow-sm hover:shadow-[0_8px_24px_-6px_rgb(244_63_94_/_0.45)] hover:-translate-y-px active:translate-y-0 transition-all duration-300',
}[props.variant]))
</script>
