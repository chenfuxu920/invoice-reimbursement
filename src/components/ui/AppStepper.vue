<template>
  <nav class="glass rounded-2xl px-4 py-3 shadow-card flex items-center gap-3 overflow-x-auto">
    <!-- 步骤 -->
    <div class="flex items-center flex-1 min-w-0">
      <template v-for="(step, i) in steps" :key="step.to">
        <button :disabled="!step.enabled" :aria-current="isCurrent(i) ? 'step' : undefined" @click="go(step.to)"
                class="group flex items-center gap-2.5 rounded-xl px-3 py-1.5 transition-all duration-200 shrink-0 disabled:cursor-not-allowed"
                :class="stepBtnClass(i)">
          <!-- 序号/勾 -->
          <span class="relative w-7 h-7 rounded-full flex items-center justify-center shrink-0 transition-all duration-300"
                :class="stepCircleClass(i)">
            <svg v-if="step.done" viewBox="0 0 24 24" class="w-4 h-4 text-white pointer-events-none">
              <path d="M5 12.5l4.5 4.5L19 7.5" fill="none" stroke="currentColor" stroke-width="3"
                    stroke-linecap="round" stroke-linejoin="round" class="check-draw" />
            </svg>
            <span v-else class="text-xs font-bold" :class="stepNumClass(i)">{{ i + 1 }}</span>
            <span v-if="isCurrent(i) && !step.done"
                  class="absolute inset-0 rounded-full ring-4 ring-primary-500/20 animate-pulse-soft" />
          </span>
          <span class="text-sm whitespace-nowrap font-medium" :class="stepTextClass(i)">{{ step.label }}</span>
        </button>
        <!-- 连接线 -->
        <div v-if="i < steps.length - 1" class="flex-1 min-w-4 h-0.5 rounded-full mx-1.5 overflow-hidden"
             :class="step.done ? 'bg-gradient-to-r from-primary-500 to-accent-500' : 'bg-slate-200'">
          <div v-if="step.done" class="h-full w-full bg-gradient-to-r from-primary-500 to-accent-500 animate-fade-in" />
        </div>
      </template>
    </div>

    <!-- 下一步 -->
    <button v-if="nextStep" class="btn-primary-glow px-4 py-2 text-sm shrink-0" @click="go(nextStep.to)">
      下一步：{{ nextStep.label }}
      <ArrowRight :size="15" />
    </button>
    <span v-else class="chip bg-emerald-50 text-emerald-700 border border-emerald-200/70 shrink-0">
      <CheckCircle2 :size="13" /> 全部完成
    </span>
  </nav>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { ArrowRight, CheckCircle2 } from 'lucide-vue-next'
import { useInvoiceStore } from '../../stores/invoice'
import { usePaymentStore } from '../../stores/payment'
import { useMatchStore } from '../../stores/match'

const router = useRouter()
const route = useRoute()
const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()

interface Step {
  to: string
  label: string
  enabled: boolean
  done: boolean
}

const steps = computed<Step[]>(() => {
  const hasInvoices = invoiceStore.invoices.length > 0
  const hasPayments = paymentStore.payments.length > 0
  const hasMatches = matchStore.matches.length > 0
  const hasTrips = matchStore.trips.length > 0
  return [
    { to: '/import', label: '收集票据', enabled: true, done: hasInvoices && hasPayments },
    { to: '/match', label: '核对匹配', enabled: hasInvoices && hasPayments, done: hasMatches },
    { to: '/export', label: '打包导出', enabled: hasMatches, done: hasTrips },
  ]
})

function go(to: string) {
  router.push(to)
}

function isCurrent(i: number) {
  return currentIndex.value === i
}

const currentIndex = computed(() => {
  const idx = steps.value.findIndex(s => route.path.startsWith(s.to))
  return idx === -1 ? 0 : idx
})

/** 下一个未完成且可用的步骤（永远提示下一步） */
const nextStep = computed(() => {
  for (let i = 0; i < steps.value.length; i++) {
    const s = steps.value[i]
    if (!s.done && (s.enabled || i === 0)) return s
  }
  return null
})

function stepBtnClass(i: number) {
  const s = steps.value[i]
  if (isCurrent(i)) return 'bg-white shadow-card border border-primary-100'
  return s.enabled ? 'hover:bg-white/70' : 'opacity-45'
}
function stepCircleClass(i: number) {
  const s = steps.value[i]
  if (s.done) return 'bg-gradient-to-br from-primary-500 to-accent-500 shadow-glow-sm'
  if (isCurrent(i)) return 'bg-gradient-to-br from-primary-500 to-accent-500 text-white shadow-glow-sm'
  return s.enabled ? 'bg-white border border-slate-300 group-hover:border-primary-400' : 'bg-slate-200'
}
function stepNumClass(i: number) {
  const s = steps.value[i]
  if (isCurrent(i)) return 'text-white'
  return s.enabled ? 'text-slate-600' : 'text-slate-400'
}
function stepTextClass(i: number) {
  if (isCurrent(i)) return 'text-slate-900'
  return steps.value[i].enabled ? 'text-slate-600' : 'text-slate-400'
}
</script>
