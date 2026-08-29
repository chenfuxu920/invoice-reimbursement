<template>
  <div class="relative inline-flex" :style="{ width: `${size}px`, height: `${size}px` }">
    <svg :width="size" :height="size" viewBox="0 0 100 100" role="img" :aria-label="ariaLabel">
      <circle cx="50" cy="50" :r="radius" fill="none" :stroke-width="stroke"
              stroke="currentColor" class="opacity-[0.18]" />
      <circle cx="50" cy="50" :r="radius" fill="none" :stroke-width="stroke" stroke="currentColor"
              stroke-linecap="round" transform="rotate(-90 50 50)"
              :stroke-dasharray="circumference" :stroke-dashoffset="dashOffset" class="donut-value" />
    </svg>
    <div class="absolute inset-0 flex flex-col items-center justify-center text-center">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  /** 百分比，可大于 100（圆环进度封顶显示为满环） */
  percent: number
  /** ok=未超标 / over=超标（颜色由父级 text-* 决定，此处仅语义） */
  tone?: 'ok' | 'over'
  size?: number
  stroke?: number
  ariaLabel?: string
}>(), { tone: 'ok', size: 72, stroke: 8, ariaLabel: '标准使用率' })

const radius = computed(() => 50 - props.stroke / 2)
const circumference = computed(() => 2 * Math.PI * radius.value)
const dashOffset = computed(() => {
  const clamped = Math.min(Math.max(props.percent, 0), 100)
  return circumference.value * (1 - clamped / 100)
})
</script>

<style scoped>
.donut-value {
  transition: stroke-dashoffset 0.7s cubic-bezier(0.22, 1, 0.36, 1);
}
@media (prefers-reduced-motion: reduce) {
  .donut-value { transition: none; }
}
</style>
