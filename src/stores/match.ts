import { defineStore } from 'pinia'
import { ref, reactive } from 'vue'
import type { MatchResult, Invoice, PaymentRecord, InvoiceCategory, ItineraryPaymentPair } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const useMatchStore = defineStore('match', () => {
  const matches = ref<MatchResult[]>([])
  const unmatchedInvoices = ref<Invoice[]>([])
  const unmatchedPayments = ref<PaymentRecord[]>([])
  const loading = ref(false)
  const reimbursementHtml = ref<string | null>(null)

  // 报销表单信息：放在 store 中跨视图持久化，避免切换导出页时组件重挂载导致城市/日期被清空
  const exportForm = reactive({
    destination: '',
    travelStart: '',
    travelEnd: '',
    hotelLevel: '其他人员',
  })

  async function autoMatch(invoices: Invoice[], payments: PaymentRecord[], tolerance = 1.0) {
    loading.value = true
    try {
      const result = await invoke<{ matched: MatchResult[]; unmatched_invoices: Invoice[]; unmatched_payments: PaymentRecord[] }>(
        'auto_match', { invoices, payments, tolerance }
      )
      matches.value = result.matched
      unmatchedInvoices.value = result.unmatched_invoices
      unmatchedPayments.value = result.unmatched_payments
    } catch (e) {
      // 后端反序列化/匹配失败时显式抛出，避免被静默吞掉（表现为"点击无反应"）
      console.error('自动匹配失败:', e)
      throw e
    } finally {
      loading.value = false
    }
  }

  function unmatchInvoice(invoiceId: string) {
    const match = matches.value.find(m => m.invoice_id === invoiceId)
    if (match) {
      matches.value = matches.value.filter(m => m.invoice_id !== invoiceId)
      unmatchedPayments.value = [...unmatchedPayments.value, ...match.payments]
      unmatchedInvoices.value = [...unmatchedInvoices.value, match.invoice]
    }
  }

  async function manualMatch(
    invoice: Invoice,
    payments: PaymentRecord[],
    itineraryPaymentPairs: ItineraryPaymentPair[] = [],
  ) {
    unmatchInvoice(invoice.id)
    const matchResult: MatchResult = await invoke('manual_match', {
      invoice,
      payments,
      itineraryPaymentPairs,
    })
    matches.value.push(matchResult)
    unmatchedInvoices.value = unmatchedInvoices.value.filter(i => i.id !== invoice.id)
    const usedIds = new Set(payments.map(p => p.id))
    unmatchedPayments.value = unmatchedPayments.value.filter(p => !usedIds.has(p.id))
  }

  function removePayment(invoiceId: string, paymentId: string) {
    const match = matches.value.find(m => m.invoice_id === invoiceId)
    if (!match) return
    const removed = match.payments.find(p => p.id === paymentId)
    if (!removed) return
    match.payments = match.payments.filter(p => p.id !== paymentId)
    match.payment_ids = match.payment_ids.filter(id => id !== paymentId)
    match.amount_diff = Math.abs(match.invoice.amount - match.payments.reduce((s, p) => s + p.amount, 0))
    if (match.payments.length === 0) {
      unmatchInvoice(invoiceId)
    } else {
      unmatchedPayments.value = [...unmatchedPayments.value, removed]
      if (match.payments.length === 1) {
        match.match_type = 'OneToOne'
      }
    }
  }

  function updateInvoiceCategory(invoiceId: string, category: InvoiceCategory) {
    const match = matches.value.find(m => m.invoice_id === invoiceId)
    if (match) {
      match.invoice = { ...match.invoice, category }
    }
    const inv = unmatchedInvoices.value.find(i => i.id === invoiceId)
    if (inv) {
      inv.category = category
    }
  }

  async function renderReimbursementHtml(formInfo: {
    name: string
    department: string
    destination: string
    travelStart: string
    travelEnd: string
    companions: number
    hotelLevel: string
  }) {
    if (matches.value.length === 0) {
      reimbursementHtml.value = null
      return
    }
    const html = await invoke<string>('render_reimbursement_html', {
      matchResults: matches.value,
      name: formInfo.name,
      department: formInfo.department,
      destination: formInfo.destination,
      travelStart: formInfo.travelStart,
      travelEnd: formInfo.travelEnd,
      companions: formInfo.companions,
      hotelLevel: formInfo.hotelLevel,
    })
    reimbursementHtml.value = html
  }

  async function saveReimbursementHtml(formInfo: {
    name: string
    department: string
    destination: string
    travelStart: string
    travelEnd: string
    companions: number
    hotelLevel: string
  }): Promise<string> {
    const outputPath = await invoke<string>('generate_reimbursement_html', {
      matchResults: matches.value,
      name: formInfo.name,
      department: formInfo.department,
      destination: formInfo.destination,
      travelStart: formInfo.travelStart,
      travelEnd: formInfo.travelEnd,
      companions: formInfo.companions,
      hotelLevel: formInfo.hotelLevel,
      outputPath: `报销单_${new Date().toISOString().slice(0, 10)}.html`,
    })
    return outputPath
  }

  function clearMatches() {
    matches.value = []
    unmatchedInvoices.value = []
    unmatchedPayments.value = []
    reimbursementHtml.value = null
    // 清空导入时一并重置报销表单，避免残留旧城市/日期
    exportForm.destination = ''
    exportForm.travelStart = ''
    exportForm.travelEnd = ''
    exportForm.hotelLevel = '其他人员'
  }

  return {
    matches, unmatchedInvoices, unmatchedPayments, loading, reimbursementHtml, exportForm,
    autoMatch, unmatchInvoice, manualMatch, removePayment, updateInvoiceCategory,
    renderReimbursementHtml, saveReimbursementHtml, clearMatches,
  }
})
