import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { MatchResult, Invoice, PaymentRecord } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const useMatchStore = defineStore('match', () => {
  const matches = ref<MatchResult[]>([])
  const unmatchedInvoices = ref<Invoice[]>([])
  const unmatchedPayments = ref<PaymentRecord[]>([])
  const loading = ref(false)

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
  }

  function clearMatches() {
    matches.value = []
    unmatchedInvoices.value = []
    unmatchedPayments.value = []
  }

  return { matches, unmatchedInvoices, unmatchedPayments, loading, autoMatch, manualMatch, clearMatches }
})
