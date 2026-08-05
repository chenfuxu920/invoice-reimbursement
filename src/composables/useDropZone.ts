import { ref, onMounted, onUnmounted } from 'vue'
import { getCurrentWebview } from '@tauri-apps/api/webview'

export const INVOICE_EXT = ['pdf', 'jpg', 'jpeg', 'png']
export const BILL_EXT = ['xlsx', 'xls', 'csv']

export function classifyPaths(paths: string[]) {
  const invoices: string[] = []
  const bills: string[] = []
  for (const p of paths) {
    const ext = p.toLowerCase().split('.').pop() || ''
    if (BILL_EXT.includes(ext)) bills.push(p)
    else invoices.push(p)
  }
  return { invoices, bills }
}

// 注册 webview 级拖放监听：窗口任意位置 drop 都会触发（含文件夹路径，展开交给 Rust collect_files）
export function useDropZone(onDrop: (invoices: string[], bills: string[]) => void) {
  const isDragging = ref(false)
  let unlisten: (() => void) | null = null

  onMounted(async () => {
    unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === 'drop' && event.payload.paths.length > 0) {
        isDragging.value = false
        const { invoices, bills } = classifyPaths(event.payload.paths)
        onDrop(invoices, bills)
      }
    })
  })

  onUnmounted(() => {
    if (unlisten) unlisten()
  })

  return { isDragging }
}
