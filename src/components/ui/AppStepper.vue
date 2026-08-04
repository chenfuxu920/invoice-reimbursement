<template>
  <nav class="flex items-center gap-2 overflow-x-auto py-1">
    <template v-for="(step, i) in steps" :key="step.to">
      <button :disabled="!step.enabled" :aria-current="isCurrent(i) ? 'step' : undefined" @click="router.push(step.to)"
              class="flex items-center gap-2 rounded-full px-3 py-1.5 text-sm whitespace-nowrap transition-colors disabled:cursor-not-allowed"
              :class="stepBtnClass(i)">
        <span class="w-5 h-5 rounded-full flex items-center justify-center text-xs font-semibold"
              :class="stepCircleClass(i)">{{ i + 1 }}</span>
        <span :class="stepTextClass(i)">{{ step.label }}</span>
      </button>
      <span v-if="i < steps.length - 1" class="w-5 h-px bg-gray-300 shrink-0" />
    </template>
  </nav>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useInvoiceStore } from '../../stores/invoice'
import { usePaymentStore } from '../../stores/payment'
import { useMatchStore } from '../../stores/match'

const router = useRouter()
const route = useRoute()
const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()

const steps = computed(() => [
  { to: '/import', label: '导入', enabled: true },
  {
    to: '/match',
    label: '匹配',
    enabled: invoiceStore.invoices.length > 0 && paymentStore.payments.length > 0,
  },
  { to: '/export', label: '导出', enabled: matchStore.matches.length > 0 },
])

function isCurrent(i: number) {
  return currentIndex.value === i
}

function stepBtnClass(i: number) {
  const active = currentIndex.value === i
  return active ? 'bg-primary-50' : steps.value[i].enabled ? 'hover:bg-gray-100' : 'opacity-50'
}
function stepCircleClass(i: number) {
  const active = currentIndex.value === i
  if (active) return 'bg-primary-600 text-white'
  return steps.value[i].enabled ? 'bg-gray-200 text-gray-600' : 'bg-gray-100 text-gray-400'
}
function stepTextClass(i: number) {
  return currentIndex.value === i ? 'text-primary-700 font-medium' : 'text-gray-600'
}

const currentIndex = computed(() => {
  const idx = steps.value.findIndex(s => route.path.startsWith(s.to))
  return idx === -1 ? 0 : idx
})
</script>
