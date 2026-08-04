<template>
  <div class="relative flex flex-col items-center justify-center py-14 text-center overflow-hidden">
    <div class="absolute -top-10 -left-10 w-40 h-40 rounded-full bg-primary-200/30 pointer-events-none" />
    <div class="absolute -bottom-12 -right-8 w-44 h-44 rounded-full bg-accent-400/20 pointer-events-none" />
    <div class="relative w-16 h-16 rounded-2xl bg-gradient-to-br from-primary-50 to-accent-400/20 border border-primary-100 flex items-center justify-center text-primary-500 mb-4 animate-float">
      <component :is="iconComponent" :size="28" />
    </div>
    <p class="relative text-sm text-slate-500 max-w-xs mb-4">{{ message }}</p>
    <div class="relative">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Inbox, Link2, Download, Bug, Search, AlertTriangle } from 'lucide-vue-next'
import type { IconName } from './AppIcon.vue'

const props = withDefaults(defineProps<{ icon?: IconName; message: string }>(), { icon: 'alert' })

const MAP: Record<IconName, unknown> = {
  home: Inbox, upload: Inbox, link: Link2, download: Download, debug: Bug,
  eye: Search, doc: Inbox, image: Inbox, table: Inbox, x: Inbox,
  'chevron-down': Search, plus: Search, spinner: Search, check: Inbox, alert: AlertTriangle,
  train: Search, plane: Search, shield: Search, swap: Search, car: Search,
  hotel: Search, meal: Search, toll: Search, clipboard: Inbox,
}

const iconComponent = computed(() => MAP[props.icon] ?? Inbox)
</script>
