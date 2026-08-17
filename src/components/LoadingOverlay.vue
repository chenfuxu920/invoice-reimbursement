<template>
  <Teleport to="body">
    <Transition name="fade">
      <div
        v-if="visible"
        class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40"
      >
        <div class="relative overflow-hidden rounded-3xl bg-white/95 shadow-card-lg px-10 py-9 flex flex-col items-center gap-5 min-w-[240px] animate-scale-in">
          <div class="absolute -top-12 -right-10 w-36 h-36 rounded-full bg-primary-400/20 pointer-events-none" />
          <span class="relative w-14 h-14 rounded-2xl bg-gradient-to-br from-primary-500 to-accent-500 shadow-glow flex items-center justify-center">
            <Loader2 class="animate-spin text-white" :size="26" />
          </span>
          <p class="relative text-slate-600 text-sm animate-pulse-soft">{{ message }}</p>
          <div v-if="progress !== undefined" class="relative w-56 h-1.5 rounded-full bg-slate-100 overflow-hidden">
            <div class="h-full rounded-full bg-gradient-to-r from-primary-500 to-accent-500 transition-all duration-300" :style="{ width: progress + '%' }" />
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { Loader2 } from 'lucide-vue-next'

withDefaults(defineProps<{
  visible: boolean
  message?: string
  progress?: number
}>(), {
  message: '处理中...'
})
</script>

<style scoped>
.fade-enter-active, .fade-leave-active {
  transition: opacity 0.3s ease;
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
}
</style>
