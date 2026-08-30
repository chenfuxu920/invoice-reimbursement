<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="visible" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-900/40" @click.self="emit('cancel')">
        <div class="bg-white/95 rounded-2xl shadow-card-lg w-full max-w-sm animate-scale-in overflow-hidden">
          <div class="px-6 pt-5 pb-4 flex items-start gap-3">
            <span class="w-10 h-10 rounded-xl bg-rose-100 text-rose-600 flex items-center justify-center shrink-0">
              <AlertTriangle :size="20" />
            </span>
            <div class="min-w-0">
              <h2 class="text-base font-bold text-slate-800">{{ title }}</h2>
              <p class="text-sm text-slate-500 mt-1.5 whitespace-pre-line leading-relaxed">{{ message }}</p>
            </div>
          </div>
          <div class="flex justify-end gap-2.5 px-6 py-4 bg-slate-50/80 border-t border-slate-100">
            <AppButton @click="emit('cancel')">{{ cancelText }}</AppButton>
            <AppButton variant="danger" @click="emit('confirm')">{{ confirmText }}</AppButton>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { watch, onUnmounted } from 'vue'
import { AlertTriangle } from 'lucide-vue-next'
import AppButton from './AppButton.vue'

const props = withDefaults(defineProps<{
  visible: boolean
  title?: string
  message: string
  confirmText?: string
  cancelText?: string
}>(), { title: '确认操作', confirmText: '确认', cancelText: '取消' })

const emit = defineEmits<{
  (e: 'confirm'): void
  (e: 'cancel'): void
}>()

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') emit('cancel')
}

watch(() => props.visible, (v) => {
  if (v) window.addEventListener('keydown', onKeydown)
  else window.removeEventListener('keydown', onKeydown)
}, { immediate: true })

onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<style scoped>
.modal-enter-active, .modal-leave-active { transition: opacity 0.25s ease; }
.modal-enter-from, .modal-leave-to { opacity: 0; }
</style>
