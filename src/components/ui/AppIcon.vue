<template>
  <svg :width="size" :height="size" viewBox="0 0 24 24" fill="none" stroke="currentColor"
       stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path v-for="d in paths" :key="d" :d="d" />
    <circle v-for="(c, i) in circles" :key="i" :cx="c[0]" :cy="c[1]" :r="c[2]" />
  </svg>
</template>

<script setup lang="ts">
import { computed } from 'vue'

export type IconName =
  | 'home' | 'upload' | 'link' | 'download' | 'debug'
  | 'eye' | 'doc' | 'image' | 'table' | 'x'
  | 'chevron-down' | 'plus' | 'spinner' | 'check' | 'alert'
  | 'train' | 'plane' | 'shield' | 'swap' | 'car'
  | 'hotel' | 'meal' | 'toll' | 'clipboard'

const props = withDefaults(defineProps<{ name: IconName; size?: number }>(), { size: 16 })

const ICONS: Record<IconName, { paths: string[]; circles?: [number, number, number][] }> = {
  home: { paths: ['M3 10.5 12 3l9 7.5', 'M5 9.5V21h14V9.5', 'M9 21v-6h6v6'] },
  upload: { paths: ['M12 16V4', 'm6 10 6-6 6 6', 'M4 20h16'] },
  link: { paths: ['M9 15l6-6', 'M11 6.5 13 4.5a4 4 0 0 1 5.7 5.7l-2 2', 'M13 17.5 11 19.5a4 4 0 0 1-5.7-5.7l2-2'] },
  download: { paths: ['M12 4v12', 'm6 10 6 6 6-6', 'M4 20h16'] },
  debug: { paths: ['M4 8l4-4M20 8l-4-4', 'M12 3v4', 'M3 12h4M17 12h4', 'M12 20v-6', 'M8 17l-3 3M16 17l3 3'] },
  eye: { paths: ['M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z', 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z'] },
  doc: { paths: ['M6 2h8l4 4v16H6z', 'M14 2v4h4', 'M9 12h6M9 16h6'] },
  image: { paths: ['M4 4h16v16H4z', 'M4 15l4-4 3 3 5-5 4 4', 'M8.5 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z'] },
  table: { paths: ['M4 4h16v16H4z', 'M4 9h16M4 14h16M10 4v16'] },
  x: { paths: ['M6 6l12 12M18 6 6 18'] },
  'chevron-down': { paths: ['m6 9 6 6 6-6'] },
  plus: { paths: ['M12 5v14M5 12h14'] },
  spinner: { paths: ['M12 3a9 9 0 1 0 9 9'] },
  check: { paths: ['m5 12 5 5L20 7'] },
  alert: { paths: ['M12 3 2 20h20L12 3Z', 'M12 10v4', 'M12 17.5v.5'] },
  train: { paths: ['M6 4h12a2 2 0 0 1 2 2v9H4V6a2 2 0 0 1 2-2Z', 'M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3', 'M8 19l-2 2M16 19l2 2', 'M8 10h8', 'M7 14h.01M17 14h.01'] },
  plane: { paths: ['M3 10.5 12 3l9 7.5', 'M12 3v9', 'M7 10.5 12 21l5-10.5'] },
  shield: { paths: ['M12 3 4 6v6c0 5 3.5 8.5 8 9 4.5-.5 8-4 8-9V6l-8-3Z', 'M9 12l2 2 4-4'] },
  swap: { paths: ['M4 7h13M17 4l3 3-3 3', 'M20 17H7M7 14l-3 3 3 3'] },
  car: { paths: ['M5 11 7 6h10l2 5', 'M4 11h16v6H4z', 'M7 17v2M17 17v2', 'M7 13h.01M17 13h.01'] },
  hotel: { paths: ['M4 21V5a1 1 0 0 1 1-1h8a1 1 0 0 1 1 1v16', 'M14 9h5a1 1 0 0 1 1 1v11', 'M2 21h20', 'M7 8h.01M10 8h.01M7 12h.01M10 12h.01'] },
  meal: { paths: ['M7 3v7a2 2 0 0 0 4 0V3', 'M9 3v18', 'M16 3c-1.5 2-1.5 6 0 8v10', 'M16 11c1.5-1 1.5-4 0-8'] },
  toll: { paths: ['M5 5h14l-4 7 4 7H5l4-7-4-7Z'] },
  clipboard: { paths: ['M9 4h6v3H9z', 'M6 4h3l6 0h3a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z'] },
}

const paths = computed(() => ICONS[props.name]?.paths ?? [])
const circles = computed(() => ICONS[props.name]?.circles ?? [])
</script>
