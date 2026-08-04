import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

const ocrOnline = ref(false)

async function refresh() {
  try { ocrOnline.value = await invoke('ocr_health') } catch { ocrOnline.value = false }
}

export function useOcrStatus() {
  return { ocrOnline, refresh }
}

export async function initOcrStatus() {
  await refresh()
  await listen('ocr-download-complete', () => { refresh() })
}
