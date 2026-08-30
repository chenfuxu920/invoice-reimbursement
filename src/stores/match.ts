import { ref, computed } from 'vue'
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
  // 待调整 = 已匹配但未归入任何趟（由 matches − trips 派生）。
  // 保证任何已匹配发票要么在趟内、要么出现在待调整区：手动补匹配、分趟未重算等状态漂移都会落到这里。
  const unassigned = computed<MatchResult[]>(() => {
    const inTrip = new Set<string>()
    for (const trip of trips.value) {
      for (const m of trip.matches) inTrip.add(m.invoice_id)
    }
    return matches.value.filter(m => !inTrip.has(m.invoice_id))
  })
  const segmentOrigin = ref('')

  function isTicket(inv: Invoice) {
    return inv.category === 'Train' || inv.category === 'Flight'
  }

  async function resegment(matchResults: MatchResult[], origin: string) {
    // 先同步到 store 状态：unassigned 由 matches − trips 派生，必须与分趟用同一份已匹配列表
    matches.value = [...matchResults]
    const result = await invoke<{ trips: TripGroupDto[]; unassigned_ids: string[] }>('segment_trips', {
      matchResults,
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
        .map(id => matches.value.find(m => m.invoice_id === id))
        .filter((m): m is MatchResult => !!m),
    }))
    // 兜底：无任何票据时全部作为单趟展示（保持原有单张导出可用）
    if (trips.value.length === 0 && !matches.value.some(m => isTicket(m.invoice))) {
      trips.value = [{
        id: 'trip-1',
        destination: '',
        travelStart: '',
        travelEnd: '',
        hotelLevel: '其他人员',
        ticketIds: [],
        matches: [...matches.value],
      }]
    }
  }

  function moveToTrip(invoiceId: string, targetTripId: string | null) {
    let match: MatchResult | undefined
    for (const trip of trips.value) {
      const idx = trip.matches.findIndex(m => m.invoice_id === invoiceId)
      if (idx >= 0) {
        match = trip.matches.splice(idx, 1)[0]
        if (isTicket(match.invoice)) {
          trip.ticketIds = trip.ticketIds.filter(id => id !== invoiceId)
        }
        break
      }
    }
    // 待调整中的发票不在任何趟里，直接从已匹配列表取（覆盖手动补匹配等未重算分趟的发票）
    if (!match) {
      match = matches.value.find(m => m.invoice_id === invoiceId)
    }
    if (!match) return
    // 目标为 null = 移出趟，留在待调整（派生列表自动包含）
    if (targetTripId === null) return
    const target = trips.value.find(t => t.id === targetTripId)
    if (target) {
      target.matches.push(match)
      if (isTicket(match.invoice) && !target.ticketIds.includes(invoiceId)) {
        target.ticketIds.push(invoiceId)
      }
    }
  }

  function createTripFromTicket(match: MatchResult) {
    // 已在某趟中的票据不重复归入；不在任何趟（含手动补匹配）的票据可直接建趟
    if (trips.value.some(t => t.matches.some(m => m.invoice_id === match.invoice_id))) return
    trips.value.push({
      id: `trip-${Date.now()}`,
      destination: match.invoice.arrival_city || '',
      travelStart: match.invoice.travel_date || '',
      travelEnd: match.invoice.travel_date || '',
      hotelLevel: '其他人员',
      ticketIds: [match.invoice_id],
      matches: [match],
    })
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
    // 同步清理趟内残留：未匹配的发票不应继续留在趟中（否则趟内展示与导出仍会带上）
    for (const trip of trips.value) {
      const idx = trip.matches.findIndex(m => m.invoice_id === invoiceId)
      if (idx >= 0) {
        trip.matches.splice(idx, 1)
        trip.ticketIds = trip.ticketIds.filter(id => id !== invoiceId)
      }
    }
  }

  async function manualMatch(
    invoice: Invoice,
    payments: PaymentRecord[],
    itineraryPaymentPairs: ItineraryPaymentPair[] = [],
  ) {
    // 记录原趟归属：调整已归趟发票的匹配后就地替换，发票留在原趟且数据保持最新
    let homeTrip: Trip | undefined
    let homeIdx = -1
    for (const trip of trips.value) {
      const idx = trip.matches.findIndex(m => m.invoice_id === invoice.id)
      if (idx >= 0) {
        homeTrip = trip
        homeIdx = idx
        break
      }
    }
    unmatchInvoice(invoice.id)
    const matchResult: MatchResult = await invoke('manual_match', {
      invoice,
      payments,
      itineraryPaymentPairs,
    })
    matches.value.push(matchResult)
    if (homeTrip) {
      homeTrip.matches.splice(Math.min(homeIdx, homeTrip.matches.length), 0, matchResult)
      if (isTicket(matchResult.invoice) && !homeTrip.ticketIds.includes(invoice.id)) {
        homeTrip.ticketIds.push(invoice.id)
      }
    }
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
    // 同步清理行程-支付配对，避免残留指向已移除支付的脏配对
    if (match.itinerary_payment_pairs?.length) {
      match.itinerary_payment_pairs = match.itinerary_payment_pairs.filter(pair => pair.payment_id !== paymentId)
    }
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
  ): Promise<string> {
    const results = matchesOverride ?? matches.value
    if (results.length === 0) {
      reimbursementHtml.value = null
      return ''
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
    return html
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
    segmentOrigin.value = ''
  }

  // 就地更新 match 中的发票（趟内/待调整引用同一 MatchResult 对象，自动同步）
  function updateMatchInvoice(updated: Invoice) {
    for (const m of matches.value) {
      if (m.invoice_id === updated.id) m.invoice = updated
    }
  }

  return {
    matches, unmatchedInvoices, unmatchedPayments, loading, reimbursementHtml,
    trips, unassigned, segmentOrigin,
    autoMatch, unmatchInvoice, manualMatch, removePayment, updateInvoiceCategory,
    renderReimbursementHtml, saveReimbursementHtml, clearMatches,
    resegment, moveToTrip, createTripFromTicket, updateMatchInvoice,
  }
})
