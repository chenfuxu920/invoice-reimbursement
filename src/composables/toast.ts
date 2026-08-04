import { ref } from 'vue'

export interface ToastItem {
  id: number
  type: 'success' | 'error' | 'info'
  message: string
}

export const toasts = ref<ToastItem[]>([])
let nextId = 1

export function toast(message: string, type: ToastItem['type'] = 'info') {
  const id = nextId++
  toasts.value.push({ id, type, message })
  setTimeout(() => removeToast(id), 3000)
}

export function removeToast(id: number) {
  toasts.value = toasts.value.filter(t => t.id !== id)
}

export function useToast() {
  return { toasts, toast, removeToast }
}
