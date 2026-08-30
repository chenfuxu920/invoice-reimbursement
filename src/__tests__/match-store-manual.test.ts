import { describe, it, expect, vi, beforeEach } from 'vitest'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))

import { createPinia, setActivePinia } from 'pinia'
import { useMatchStore } from '../stores/match'
import type { Invoice, MatchResult, PaymentRecord, ItineraryPaymentPair } from '../types'

function makeInvoice(id: string, amount = 100, itineraryCount = 0): Invoice {
  return {
    id,
    invoice_number: `INV-${id}`,
    amount,
    seller_name: '',
    item_name: '',
    date: '2026-05-20',
    category: 'CityTransport',
    source: { type: 'Manual' },
    itineraries: Array.from({ length: itineraryCount }, (_, i) => ({
      city: '',
      date_time: `2026-05-20 0${i + 1}:00`,
      provider: '滴滴',
      pickup: 'A',
      dropoff: 'B',
      amount: 30,
      incomplete_fields: [],
    })),
  }
}

function makePayment(id: string, amount: number): PaymentRecord {
  return {
    id,
    transaction_id: `TX-${id}`,
    transaction_time: '2026-05-20 09:00',
    amount,
    original_amount: amount,
    refund_amount: 0,
    discount: 0,
    merchant_name: '商户',
    source: 'Wechat',
    category: '交通',
    payment_method: '',
  }
}

function makeMatch(
  invoice: Invoice,
  payments: PaymentRecord[],
  pairs: ItineraryPaymentPair[] = [],
): MatchResult {
  return {
    invoice_id: invoice.id,
    invoice,
    payment_ids: payments.map(p => p.id),
    payments,
    match_type: payments.length > 1 ? 'OneToMany' : 'OneToOne',
    confidence: 1,
    amount_diff: 0,
    itinerary_payment_pairs: pairs,
  }
}

describe('matchStore 手动匹配抢占已用支付', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invokeMock.mockReset()
  })

  it('选中其他发票的支付：从原发票匹配移除并更新配对与金额差，原发票保留剩余支付', async () => {
    const store = useMatchStore()
    const p1 = makePayment('p1', 30)
    const p2 = makePayment('p2', 40)
    // 发票A：2条行程，分别配对 p1/p2
    const invA = makeInvoice('invA', 70, 2)
    const matchA = makeMatch(invA, [p1, p2], [
      { itinerary_index: 0, payment_id: 'p1' },
      { itinerary_index: 1, payment_id: 'p2' },
    ])
    matchA.amount_diff = 0
    store.matches = [matchA]
    store.unmatchedInvoices = []
    const invB = makeInvoice('invB', 40)

    invokeMock.mockResolvedValue(makeMatch(invB, [p2]))
    await store.manualMatch(invB, [p2])

    // 发票B 匹配成功，占用 p2
    expect(store.matches.some(m => m.invoice_id === 'invB')).toBe(true)
    // 发票A 保留 p1：配对清理为仅行程0，金额差 = 70 - 30 = 40
    const matchAAfter = store.matches.find(m => m.invoice_id === 'invA')
    expect(matchAAfter).toBeTruthy()
    expect(matchAAfter!.payment_ids).toEqual(['p1'])
    expect(matchAAfter!.itinerary_payment_pairs).toEqual([{ itinerary_index: 0, payment_id: 'p1' }])
    expect(matchAAfter!.amount_diff).toBeCloseTo(40)
    expect(matchAAfter!.match_type).toBe('OneToOne')
    // 发票A 未被整张取消，未出现在未匹配列表
    expect(store.unmatchedInvoices).toHaveLength(0)
    // p2 本就不在未匹配支付列表，列表不变
    expect(store.unmatchedPayments).toHaveLength(0)
  })

  it('原发票被夺走全部支付后整张回到未匹配列表', async () => {
    const store = useMatchStore()
    const p1 = makePayment('p1', 30)
    const invA = makeInvoice('invA', 30)
    const matchA = makeMatch(invA, [p1])
    store.matches = [matchA]
    const invB = makeInvoice('invB', 30)

    invokeMock.mockResolvedValue(makeMatch(invB, [p1]))
    await store.manualMatch(invB, [p1])

    expect(store.matches.some(m => m.invoice_id === 'invA')).toBe(false)
    expect(store.unmatchedInvoices.map(i => i.id)).toContain('invA')
    expect(store.matches.some(m => m.invoice_id === 'invB')).toBe(true)
  })

  it('调整自身发票的匹配不会触发抢占', async () => {
    const store = useMatchStore()
    const p1 = makePayment('p1', 30)
    const p2 = makePayment('p2', 50)
    const invA = makeInvoice('invA', 80)
    const matchA = makeMatch(invA, [p1])
    store.matches = [matchA]
    store.unmatchedPayments = [p2]

    // 用未匹配的 p2 + 自己的 p1 重新匹配自己
    invokeMock.mockResolvedValue(makeMatch(invA, [p1, p2]))
    await store.manualMatch(invA, [p1, p2])

    // p1 是自身支付，不应走抢占路径；unmatchInvoice 已释放 p1，重新匹配后回收
    const newMatch = store.matches.find(m => m.invoice_id === 'invA')
    expect(newMatch).toBeTruthy()
    expect(newMatch!.payment_ids).toEqual(['p1', 'p2'])
    expect(store.unmatchedPayments).toHaveLength(0)
    expect(store.unmatchedInvoices).toHaveLength(0)
  })

  it('removePayment 移除行程发票的支付时同步清理对应配对', () => {
    const store = useMatchStore()
    const p1 = makePayment('p1', 30)
    const p2 = makePayment('p2', 40)
    const invA = makeInvoice('invA', 70, 2)
    const matchA = makeMatch(invA, [p1, p2], [
      { itinerary_index: 0, payment_id: 'p1' },
      { itinerary_index: 1, payment_id: 'p2' },
    ])
    store.matches = [matchA]
    store.unmatchedPayments = []

    store.removePayment('invA', 'p2')

    expect(matchA.payment_ids).toEqual(['p1'])
    expect(matchA.itinerary_payment_pairs).toEqual([{ itinerary_index: 0, payment_id: 'p1' }])
    expect(matchA.amount_diff).toBeCloseTo(40)
    // 被移除的支付回到未匹配列表
    expect(store.unmatchedPayments.map(p => p.id)).toContain('p2')
  })
})
