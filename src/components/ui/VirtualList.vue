<template>
  <div ref="parentRef" v-bind="$attrs" class="overflow-auto">
    <div :style="{ height: `${virtualizer.getTotalSize()}px` }" class="relative w-full">
      <div
        v-for="v in virtualizer.getVirtualItems()"
        :key="v.index"
        :data-index="v.index"
        :ref="(el) => virtualizer.measureElement(el as Element | null)"
        class="absolute top-0 left-0 w-full"
        :style="{ transform: `translateY(${v.start}px)` }"
      >
        <slot :item="items[v.index]" :index="v.index" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts" generic="T">
import { ref, computed } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'

const props = withDefaults(defineProps<{
  items: T[]
  estimateSize?: number
  overscan?: number
}>(), {
  estimateSize: 120,
  overscan: 8,
})

defineSlots<{
  default: (props: { item: T; index: number }) => unknown
}>()

const parentRef = ref<HTMLElement | null>(null)
// 整个 options 包成 computed：items 变化时 virtualizer 自动更新 count，无需手动重建
const virtualizer = useVirtualizer(computed(() => ({
  count: props.items.length,
  getScrollElement: () => parentRef.value,
  estimateSize: () => props.estimateSize,
  overscan: props.overscan,
})))
</script>
