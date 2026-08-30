import { describe, it, expect } from 'vitest'
import { countUnmatchedItineraries, hasExportGaps } from '../utils/match'
import type { MatchResult, Invoice, ItineraryPaymentPair } from '../types'

function makeInvoice(id: string, itineraryCount: number): Invoice {
  return {
    id,
    invoice_number: `INV-${id}`,
    amount: 100,
    seller_name: '',
    item_name: '',
    date: '2026-05-20',
    category: 'CityTransport',
    source: { type: 'Manual' },
    itineraries: Array.from({ length: itineraryCount }, (_, i) => ({
      city: '',
      date_time: `2026-05-20 0${i}:00`,
      provider: '滴滴',
      pickup: 'A',
      dropoff: 'B',
      amount: 30,
      incomplete_fields: [],
    })),
  }
}

function makeMatch(id: string, itineraryCount: number, pairs: ItineraryPaymentPair[]): MatchResult {
  return {
    invoice_id: id,
    invoice: makeInvoice(id, itineraryCount),
    payment_ids: pairs.map(p => p.payment_id),
    payments: [],
    match_type: 'OneToMany',
    confidence: 1,
    amount_diff: 0,
    itinerary_payment_pairs: pairs,
  }
}

describe('countUnmatchedItineraries', () => {
  it('无行程发票返回 0', () => {
    expect(countUnmatchedItineraries(makeMatch('m1', 0, []))).toBe(0)
  })

  it('全量配对返回 0', () => {
    const m = makeMatch('m1', 2, [
      { itinerary_index: 0, payment_id: 'p1' },
      { itinerary_index: 1, payment_id: 'p2' },
    ])
    expect(countUnmatchedItineraries(m)).toBe(0)
  })

  it('部分配对统计缺口的行程数', () => {
    const m = makeMatch('m1', 3, [{ itinerary_index: 0, payment_id: 'p1' }])
    expect(countUnmatchedItineraries(m)).toBe(2)
  })

  it('配对表为空（旧数据/整单支付）无法判定，返回 0', () => {
    expect(countUnmatchedItineraries(makeMatch('m1', 3, []))).toBe(0)
  })

  it('itinerary_payment_pairs 缺失字段时按旧数据处理', () => {
    const m = makeMatch('m1', 2, [])
    delete (m as Partial<MatchResult>).itinerary_payment_pairs
    expect(countUnmatchedItineraries(m)).toBe(0)
  })
})

describe('hasExportGaps', () => {
  it('有未匹配发票时返回 true', () => {
    expect(hasExportGaps(1, [])).toBe(true)
  })

  it('有未配对行程时返回 true', () => {
    const m = makeMatch('m1', 2, [{ itinerary_index: 1, payment_id: 'p1' }])
    expect(hasExportGaps(0, [m])).toBe(true)
  })

  it('全部匹配完整时返回 false', () => {
    const m = makeMatch('m1', 1, [{ itinerary_index: 0, payment_id: 'p1' }])
    expect(hasExportGaps(0, [m])).toBe(false)
    expect(hasExportGaps(0, [])).toBe(false)
  })
})
