import { ref } from 'vue'
import { defineStore } from 'pinia'
import type { MatchResult, Invoice, PaymentRecord, InvoiceCategory, ItineraryPaymentPair, Trip } from '../types'
import { invoke } from '@tauri-apps/api/core'

export const useMatchStore = defineStore('match', () => {
  const matches = ref<MatchResult[]>([])
  const unmatchedInvoices = ref<Invoice[]>([])
  const unmatchedPayments = ref<PaymentRecord[]>([])
  const loading = ref(false)
  const reimbursementHtml = ref<string | null>(null)

  // 报销表单信息：放在 store 中跨视图持久化，避免切换导出页时组件重挂载导致城市/日期被清空

  // 后端 segment_trips 返回的趟分组（snake_case）
  interface TripGroupDto {
    id: string
    destination: string
    travel_start: string
    travel_end: string
    ticket_ids: string[]
    invoice_ids: string[]
  }

  const trips = ref<Trip[]>([])
  const unassigned = ref<MatchResult[]>([])
  const segmentOrigin = ref('')

  function isTicket(inv: Invoice) {
    return inv.category === 'Train' || inv.category === 'Flight'
  }

  async function resegment(matches: MatchResult[], origin: string) {
    const result = await invoke<{ trips: TripGroupDto[]; unassigned_ids: string[] }>('segment_trips', {
      matchResults: matches,
      origin: origin || null,
    })
    trips.value = result.trips.map(t => ({
      id: t.id,
      destination: t.destination,
      travelStart: t.travel_start,
      travelEnd: t.travel_end,
      hotelLevel: '其他人员',
      ticketIds: t.ticket_ids,
      matches: t.invoice_ids
        .map(id => matches.find(m => m.invoice_id === id))
        .filter((m): m is MatchResult => !!m),
    }))
    unassigned.value = result.unassigned_ids
      .map(id => matches.find(m => m.invoice_id === id))
      .filter((m): m is MatchResult => !!m)
    // 兜底：无任何票据时全部作为单趟展示（保持原有单张导出可用）
    if (trips.value.length === 0 && !matches.some(m => isTicket(m.invoice))) {
      trips.value = [{
        id: 'trip-1',
        destination: '',
        travelStart: '',
        travelEnd: '',
        hotelLevel: '其他人员',
        ticketIds: [],
        matches,
      }]
      unassigned.value = []
    }
  }

  function moveToTrip(invoiceId: string, targetTripId: string | null) {
    let match: MatchResult | undefined
    for (const trip of trips.value) {
      const idx = trip.matches.findIndex(m => m.invoice_id === invoiceId)
      if (idx >= 0) {
        match = trip.matches.splice(idx, 1)[0]
        break
      }
    }
    if (!match) {
      const idx = unassigned.value.findIndex(m => m.invoice_id === invoiceId)
      if (idx >= 0) match = unassigned.value.splice(idx, 1)[0]
    }
    if (!match) return
    if (targetTripId === null) {
      unassigned.value.push(match)
      return
    }
    const target = trips.value.find(t => t.id === targetTripId)
    if (target) target.matches.push(match)
  }

  function createTripFromTicket(match: MatchResult) {
    trips.value.push({
      id: `trip-${Date.now()}`,
      destination: match.invoice.arrival_city || '',
      travelStart: match.invoice.travel_date || '',
      travelEnd: match.invoice.travel_date || '',
      hotelLevel: '其他人员',
      ticketIds: [match.invoice_id],
      matches: [match],
    })
    unassigned.value = unassigned.value.filter(m => m.invoice_id !== match.invoice_id)
  }

  async function autoMatch(invoices: Invoice[], payments: PaymentRecord[], tolerance = 1.0) {
    loading.value = true
    try {
      const result = await invoke<{ matched: MatchResult[]; unmatched_invoices: Invoice[]; unmatched_payments: PaymentRecord[] }>(
        'auto_match', { invoices, payments, tolerance }
      )
      matches.value = result.matched
      unmatchedInvoices.value = result.unmatched_invoices
      unmatchedPayments.value = result.unmatched_payments
      await resegment(matches.value, segmentOrigin.value)
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

  async function renderReimbursementHtml(
    formInfo: {
      name: string
      department: string
      destination: string
      travelStart: string
      travelEnd: string
      companions: number
      hotelLevel: string
    },
    matchesOverride?: MatchResult[],
  ) {
    const results = matchesOverride ?? matches.value
    if (results.length === 0) {
      reimbursementHtml.value = null
      return
    }
    const html = await invoke<string>('render_reimbursement_html', {
      matchResults: results,
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
    trips.value = []
    unassigned.value = []
    segmentOrigin.value = ''
  }

  return {
    matches, unmatchedInvoices, unmatchedPayments, loading, reimbursementHtml,
    trips, unassigned, segmentOrigin,
    autoMatch, unmatchInvoice, manualMatch, removePayment, updateInvoiceCategory,
    renderReimbursementHtml, saveReimbursementHtml, clearMatches,
    resegment, moveToTrip, createTripFromTicket,
  }
})
