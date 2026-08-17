<template>
  <Teleport to="body">
    <div class="fixed top-4 right-4 z-[60] space-y-2 w-96" role="status" aria-live="polite">
      <TransitionGroup name="toast">
        <div v-for="t in toasts" :key="t.id"
             class="flex items-start gap-3 rounded-2xl border bg-white/85 p-3.5 shadow-card-lg"
             :class="borderClass(t.type)">
          <span class="w-8 h-8 rounded-xl flex items-center justify-center shrink-0" :class="iconWrapClass(t.type)">
            <component :is="iconFor(t.type)" :size="16" />
          </span>
          <p class="text-sm text-slate-700 flex-1 break-words whitespace-pre-line leading-relaxed pt-1">{{ t.message }}</p>
          <button v-if="t.action" class="text-primary-600 font-medium hover:underline text-xs shrink-0 mt-1 transition-colors" @click="handleAction(t)">
            {{ t.action.label }}
          </button>
          <button class="text-slate-400 hover:text-slate-600 shrink-0 mt-1 transition-colors" :aria-label="'关闭提示'" @click="removeToast(t.id)">
            <X :size="14" />
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { X, CheckCircle2, AlertCircle, Info } from 'lucide-vue-next'
import { useToast } from '../../composables/toast'
import type { ToastItem } from '../../composables/toast'

const { toasts, removeToast } = useToast()

function handleAction(t: ToastItem) {
  t.action?.onClick()
  removeToast(t.id)
}

function borderClass(type: ToastItem['type']) {
  return { success: 'border-emerald-200/80', error: 'border-rose-200/80', info: 'border-primary-200/80' }[type]
}
function iconWrapClass(type: ToastItem['type']) {
  return {
    success: 'bg-emerald-100 text-emerald-600',
    error: 'bg-rose-100 text-rose-600',
    info: 'bg-primary-100 text-primary-600',
  }[type]
}
function iconFor(type: ToastItem['type']) {
  return { success: CheckCircle2, error: AlertCircle, info: Info }[type]
}
</script>

<style scoped>
.toast-enter-active, .toast-leave-active { transition: all 0.3s cubic-bezier(0.22, 1, 0.36, 1); }
.toast-enter-from { opacity: 0; transform: translateX(24px) scale(0.96); }
.toast-leave-to { opacity: 0; transform: translateX(24px) scale(0.96); }
</style>
