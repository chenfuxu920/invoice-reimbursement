<template>
  <Teleport to="body">
    <div v-if="visible" class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4" @click.self="emit('cancel')">
      <div class="bg-white rounded-[10px] shadow-2xl w-full max-w-sm">
        <div class="px-5 py-4 border-b border-gray-100">
          <h2 class="text-base font-semibold text-gray-800">{{ title }}</h2>
        </div>
        <div class="px-5 py-4">
          <p class="text-sm text-gray-600 whitespace-pre-line">{{ message }}</p>
        </div>
        <div class="flex justify-end gap-2 px-5 py-3 border-t border-gray-100">
          <AppButton @click="emit('cancel')">取消</AppButton>
          <AppButton variant="danger" @click="emit('confirm')">{{ confirmText }}</AppButton>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { watch, onUnmounted } from 'vue'
import AppButton from './AppButton.vue'

const props = withDefaults(defineProps<{
  visible: boolean
  title?: string
  message: string
  confirmText?: string
}>(), { title: '确认操作', confirmText: '确认' })

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
