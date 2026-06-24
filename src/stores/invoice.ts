import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Invoice, InvoiceCategory, ParseError } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const useInvoiceStore = defineStore('invoice', () => {
  const invoices = ref<Invoice[]>([])
  const loading = ref(false)
  const parseErrors = ref<ParseError[]>([])

  async function addInvoice(filePath: string, fileType: string): Promise<boolean> {
    loading.value = true
    try {
      const invoice: Invoice = await invoke('recognize_invoice', { filePath, fileType })
      // 跨批次去重：有发票号且已存在则跳过
      if (invoice.invoice_number && invoices.value.some(i => i.invoice_number === invoice.invoice_number)) {
        return false
      }
      invoices.value.push(invoice)
      return true
    } catch (e) {
      console.error('识别发票失败:', e)
      throw e
    } finally {
      loading.value = false
    }
  }

  /// 批量追加发票，跳过与已有列表发票号重复的项；
  /// 无发票号的项放行（与后端 dedup 语义一致）。
  /// 返回被跳过的发票号列表。
  function addInvoicesSkipDuplicates(newInvoices: Invoice[]): string[] {
    const skipped: string[] = []
    for (const inv of newInvoices) {
      if (inv.invoice_number && invoices.value.some(i => i.invoice_number === inv.invoice_number)) {
        skipped.push(inv.invoice_number)
        continue
      }
      invoices.value.push(inv)
    }
    return skipped
  }

  function removeInvoice(id: string) {
    invoices.value = invoices.value.filter(i => i.id !== id)
  }

  function updateCategory(invoiceId: string, category: InvoiceCategory) {
    const invoice = invoices.value.find(i => i.id === invoiceId)
    if (invoice) {
      invoice.category = category
    }
  }

  function clearInvoices() {
    invoices.value = []
    parseErrors.value = []
  }

  function addParseErrors(errors: ParseError[]) {
    parseErrors.value.push(...errors)
  }

  function removeParseError(id: string) {
    parseErrors.value = parseErrors.value.filter(e => e.id !== id)
  }

  function clearParseErrors() {
    parseErrors.value = []
  }

  function addManualInvoice(invoice: Invoice) {
    invoices.value.push(invoice)
  }

  return { invoices, loading, parseErrors, addInvoice, addInvoicesSkipDuplicates, removeInvoice, updateCategory, clearInvoices, addParseErrors, removeParseError, clearParseErrors, addManualInvoice }
})
