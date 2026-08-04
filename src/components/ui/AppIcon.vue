<template>
  <component :is="iconComponent" :size="size" :stroke-width="1.8" aria-hidden="true" />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  Home, Upload, Link2, Download, Bug, Eye, FileText, Image as ImageIcon,
  Table2, X, ChevronDown, Plus, Loader2, Check, AlertTriangle,
  TrainFront, Plane, ShieldCheck, ArrowLeftRight, Car, BedDouble,
  UtensilsCrossed, CircleDollarSign, ClipboardList,
} from 'lucide-vue-next'

export type IconName =
  | 'home' | 'upload' | 'link' | 'download' | 'debug'
  | 'eye' | 'doc' | 'image' | 'table' | 'x'
  | 'chevron-down' | 'plus' | 'spinner' | 'check' | 'alert'
  | 'train' | 'plane' | 'shield' | 'swap' | 'car'
  | 'hotel' | 'meal' | 'toll' | 'clipboard'

const props = withDefaults(defineProps<{ name: IconName; size?: number }>(), { size: 16 })

const ICONS: Record<IconName, unknown> = {
  home: Home,
  upload: Upload,
  link: Link2,
  download: Download,
  debug: Bug,
  eye: Eye,
  doc: FileText,
  image: ImageIcon,
  table: Table2,
  x: X,
  'chevron-down': ChevronDown,
  plus: Plus,
  spinner: Loader2,
  check: Check,
  alert: AlertTriangle,
  train: TrainFront,
  plane: Plane,
  shield: ShieldCheck,
  swap: ArrowLeftRight,
  car: Car,
  hotel: BedDouble,
  meal: UtensilsCrossed,
  toll: CircleDollarSign,
  clipboard: ClipboardList,
}

const iconComponent = computed(() => ICONS[props.name])
</script>
