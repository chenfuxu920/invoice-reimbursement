import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Invoice } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const useInvoiceStore = defineStore('invoice', () => {
  const invoices = ref<Invoice[]>([])
  const loading = ref(false)

  async function addInvoice(filePath: string, fileType: string) {
    loading.value = true
    try {
      const invoice: Invoice = await invoke('recognize_invoice', { filePath, fileType })
      invoices.value.push(invoice)
    } catch (e) {
      console.error('识别发票失败:', e)
      throw e
    } finally {
      loading.value = false
    }
  }

  function removeInvoice(id: string) {
    invoices.value = invoices.value.filter(i => i.id !== id)
  }

  return { invoices, loading, addInvoice, removeInvoice }
})
