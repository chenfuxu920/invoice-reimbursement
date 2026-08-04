<template>
  <Teleport to="body">
    <div class="fixed top-4 right-4 z-[60] space-y-2 w-80">
      <TransitionGroup name="toast">
        <div v-for="t in toasts" :key="t.id"
             class="flex items-start gap-2.5 rounded-lg border bg-white p-3 shadow-lg"
             :class="borderClass(t.type)">
          <span class="w-2 h-2 rounded-full mt-1.5 shrink-0" :class="dotClass(t.type)" />
          <p class="text-sm text-gray-700 flex-1 break-words">{{ t.message }}</p>
          <button class="text-gray-400 hover:text-gray-600 shrink-0" :aria-label="'关闭提示'" @click="removeToast(t.id)">
            <AppIcon name="x" :size="14" />
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import AppIcon from './AppIcon.vue'
import { useToast } from '../../composables/toast'
import type { ToastItem } from '../../composables/toast'

const { toasts, removeToast } = useToast()

function borderClass(type: ToastItem['type']) {
  return { success: 'border-emerald-200', error: 'border-red-200', info: 'border-primary-200' }[type]
}
function dotClass(type: ToastItem['type']) {
  return { success: 'bg-emerald-500', error: 'bg-red-500', info: 'bg-primary-500' }[type]
}
</script>

<style scoped>
.toast-enter-active, .toast-leave-active { transition: all 0.25s ease; }
.toast-enter-from { opacity: 0; transform: translateX(16px); }
.toast-leave-to { opacity: 0; transform: translateX(16px); }
</style>
