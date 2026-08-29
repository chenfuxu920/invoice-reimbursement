import { describe, it, expect } from 'vitest'
import { analyzeStayDays } from '../utils/stay'
import type { Invoice } from '../types/invoice'
import type { MatchResult, Trip } from '../types/match'

let seq = 0

function makeInvoice(partial: Partial<Invoice>): Invoice {
  seq += 1
  return {
    id: partial.id ?? `inv-${seq}`,
    invoice_number: '',
    amount: 0,
    seller_name: '',
    item_name: '',
    date: '2025-08-04',
    category: 'Other',
    source: { type: 'Manual' },
    itineraries: [],
    ...partial,
  }
}

function makeMatch(invoice: Invoice): MatchResult {
  return {
    invoice_id: invoice.id,
    invoice,
    payment_ids: [],
    payments: [],
    match_type: 'Unmatched',
    confidence: 0,
    amount_diff: 0,
  }
}

function makeTrip(partial: Partial<Trip>, invoices: Invoice[]): Trip {
  return {
    id: `trip-${++seq}`,
    destination: '长沙',
    travelStart: '2025-08-04',
    travelEnd: '2025-08-08',
    hotelLevel: '',
    ticketIds: [],
    matches: invoices.map(makeMatch),
    ...partial,
  }
}

function hotelInvoice(nights: number, amount = 300): Invoice {
  return makeInvoice({
    category: 'Hotel',
    amount,
    hotel_detail: nights > 0
      ? { check_in: '2025-08-04', check_out: '2025-08-09', nights, nightly_rate: amount / nights }
      : null,
  })
}

function transportInvoice(): Invoice {
  return makeInvoice({ category: 'Train', amount: 100 })
}

describe('analyzeStayDays', () => {
  it('没有住宿发票时无需核对', () => {
    expect(analyzeStayDays(makeTrip({}, [transportInvoice()]))).toBeNull()
    expect(analyzeStayDays(makeTrip({}, []))).toBeNull()
  })

  it('行程日期不完整或非法时跳过核对', () => {
    expect(analyzeStayDays(makeTrip({ travelStart: '' }, [hotelInvoice(4)]))).toBeNull()
    expect(analyzeStayDays(makeTrip({ travelEnd: '08-08' }, [hotelInvoice(4)]))).toBeNull()
  })

  it('住宿晚数与行程对应（N 天 N-1 晚）时无提示', () => {
    // 5 天 4 晚
    expect(analyzeStayDays(makeTrip({}, [hotelInvoice(4)]))).toBeNull()
    // 多张住宿发票晚数合计对应（换酒店场景：2 晚 + 2 晚）
    expect(analyzeStayDays(makeTrip({}, [hotelInvoice(2), hotelInvoice(2)]))).toBeNull()
  })

  it('住宿晚数与行程不符时返回 mismatch', () => {
    expect(analyzeStayDays(makeTrip({}, [hotelInvoice(3)]))).toEqual({
      status: 'mismatch',
      tripDays: 5,
      expectedNights: 4,
      nights: 3,
      unknownCount: 0,
    })
    // 多张合计超出（含重复/同行场景）
    const r = analyzeStayDays(makeTrip({}, [hotelInvoice(4), hotelInvoice(4)]))
    expect(r).toMatchObject({ status: 'mismatch', nights: 8, expectedNights: 4 })
  })

  it('住宿发票缺少入住/离店明细时返回 incomplete', () => {
    expect(analyzeStayDays(makeTrip({}, [hotelInvoice(0)]))).toEqual({
      status: 'incomplete',
      tripDays: 5,
      expectedNights: 4,
      nights: 0,
      unknownCount: 1,
    })
    // 部分缺明细：即使已知晚数看起来不足也不判 mismatch，避免误报
    const r = analyzeStayDays(makeTrip({}, [hotelInvoice(2), hotelInvoice(0)]))
    expect(r).toMatchObject({ status: 'incomplete', nights: 2, unknownCount: 1 })
  })

  it('非住宿发票不影响核对', () => {
    expect(analyzeStayDays(makeTrip({}, [hotelInvoice(4), transportInvoice()]))).toBeNull()
  })

  it('单日行程带住宿发票视为不符', () => {
    expect(analyzeStayDays(makeTrip({ travelStart: '2025-08-04', travelEnd: '2025-08-04' }, [hotelInvoice(1)])))
      .toMatchObject({ status: 'mismatch', tripDays: 1, expectedNights: 0, nights: 1 })
  })

  it('日期倒挂时按后端口径钳制为 1 天', () => {
    expect(analyzeStayDays(makeTrip({ travelStart: '2025-08-08', travelEnd: '2025-08-04' }, [hotelInvoice(4)])))
      .toMatchObject({ status: 'mismatch', tripDays: 1, expectedNights: 0, nights: 4 })
  })
})
