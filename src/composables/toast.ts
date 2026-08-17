import { ref } from 'vue'

export interface ToastAction {
  label: string
  onClick: () => void
}

export interface ToastItem {
  id: number
  type: 'success' | 'error' | 'info'
  message: string
  action?: ToastAction
}

export const toasts = ref<ToastItem[]>([])
let nextId = 1

export function toast(message: string, type: ToastItem['type'] = 'info', action?: ToastAction) {
  const id = nextId++
  toasts.value.push({ id, type, message, action })
  // 带动作按钮时给用户更长的操作时间
  setTimeout(() => removeToast(id), action ? 6000 : 3000)
}

export function removeToast(id: number) {
  toasts.value = toasts.value.filter(t => t.id !== id)
}

export function useToast() {
  return { toasts, toast, removeToast }
}
