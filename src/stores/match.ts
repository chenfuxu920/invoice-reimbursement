import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { MatchResult, Invoice, PaymentRecord } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const useMatchStore = defineStore('match', () => {
  const matches = ref<MatchResult[]>([])
  const unmatchedInvoices = ref<Invoice[]>([])
  const unmatchedPayments = ref<PaymentRecord[]>([])
  const loading = ref(false)
  const reimbursementHtml = ref<string | null>(null)

  async function autoMatch(invoices: Invoice[], payments: PaymentRecord[], tolerance = 1.0) {
    loading.value = true
    try {
      const result = await invoke<{ matched: MatchResult[]; unmatched_invoices: Invoice[]; unmatched_payments: PaymentRecord[] }>(
        'auto_match', { invoices, payments, tolerance }
      )
      matches.value = result.matched
      unmatchedInvoices.value = result.unmatched_invoices
      unmatchedPayments.value = result.unmatched_payments
    } finally {
      loading.value = false
    }
  }

  async function manualMatch(invoice: Invoice, payments: PaymentRecord[]) {
    const matchResult: MatchResult = await invoke('manual_match', { invoice, payments })
    matches.value.push(matchResult)
    unmatchedInvoices.value = unmatchedInvoices.value.filter(i => i.id !== invoice.id)
    const usedIds = new Set(payments.map(p => p.id))
    unmatchedPayments.value = unmatchedPayments.value.filter(p => !usedIds.has(p.id))
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
  }

  return {
    matches, unmatchedInvoices, unmatchedPayments, loading, reimbursementHtml,
    autoMatch, manualMatch, renderReimbursementHtml, saveReimbursementHtml, clearMatches,
  }
})
