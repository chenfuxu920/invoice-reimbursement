import { ref } from 'vue'

// 首页拖入的文件暂存：跳转到导入页后由 ImportView 消费并清空
export const pendingDrop = ref<{ invoices: string[]; bills: string[] } | null>(null)

export function consumePendingDrop() {
  const drop = pendingDrop.value
  pendingDrop.value = null
  return drop
}
