import { describe, it, expect } from 'vitest'
import { analyzeTripOverage } from '../utils/overage'
import type { Invoice, Itinerary } from '../types/invoice'
import type { MatchResult, Trip } from '../types/match'
import type { ReimbursementForm } from '../types/reimbursement'

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
    travel_date: undefined,
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
    match_type: 'OneToOne',
    confidence: 1,
    amount_diff: 0,
  }
}

function makeTrip(matches: Invoice[]): Trip {
  return {
    id: 'trip-1',
    destination: '成都',
    travelStart: '2025-08-04',
    travelEnd: '2025-08-09',
    hotelLevel: '其他人员',
    ticketIds: [],
    matches: matches.map(makeMatch),
  }
}

function makeForm(partial: Partial<ReimbursementForm>): ReimbursementForm {
  return {
    name: '',
    department: '',
    destination: '成都',
    travel_start: '2025-08-04',
    travel_end: '2025-08-09',
    travel_days: 6,
    companions: 0,
    transport_details: [],
    transport_subtotal: 0,
    city_transport_count: 0,
    city_transport_amount: 0,
    city_transport_actual_amount: 0,
    city_transport_daily_std: 80,
    hotel_levels: [],
    hotel_subtotal: 0,
    meal_subsidy: { persons: 1, days: 6, daily_rate: 100, amount: 600 },
    baggage_amount: 0,
    meal_reimbursement: 0,
    summaries: [],
    total_amount: 0,
    ...partial,
  }
}

function makeRide(partial: Partial<Itinerary>): Itinerary {
  return {
    date_time: '2025-08-05 10:30',
    provider: '滴滴',
    pickup: '酒店',
    dropoff: '客户现场',
    amount: 0,
    incomplete_fields: [],
    ...partial,
  }
}

describe('analyzeTripOverage', () => {
  it('无封顶类费用时两类目均为 null 且不超标', () => {
    const trip = makeTrip([makeInvoice({ amount: 553, category: 'Train' })])
    const result = analyzeTripOverage(trip, makeForm({}))
    expect(result.cityTransport).toBeNull()
    expect(result.hotel).toBeNull()
    expect(result.over).toBe(false)
    expect(result.overTotal).toBe(0)
  })

  it('均未超标时各类目分开计算使用率且无建议', () => {
    const trip = makeTrip([
      makeInvoice({ amount: 400, category: 'CityTransport' }),
      makeInvoice({ amount: 1500, category: 'Hotel', hotel_detail: { nights: 5, nightly_rate: 300, check_in: '2025-08-04', check_out: '2025-08-09' } }),
    ])
    const result = analyzeTripOverage(trip, makeForm({
      city_transport_actual_amount: 400,
      city_transport_amount: 400,
      hotel_levels: [{ level: '其他人员', persons: 1, days: 5, daily_rate: 350, amount: 1500, actual_amount: 1500 }],
    }))
    expect(result.over).toBe(false)
    expect(result.cityTransport!.usageRate).toBeCloseTo((400 / 480) * 100, 6)
    expect(result.cityTransport!.over).toBe(false)
    expect(result.cityTransport!.suggestedInvoices).toHaveLength(0)
    expect(result.hotel!.usageRate).toBeCloseTo((1500 / 1750) * 100, 6)
    expect(result.hotel!.over).toBe(false)
    expect(result.hotel!.items).toHaveLength(0)
  })

  it('市内交通超标：按最小移交金额选发票（220+200=420 覆盖超额 260）', () => {
    const trip = makeTrip([
      makeInvoice({ amount: 553, category: 'Train' }),
      makeInvoice({ amount: 240, category: 'CityTransport', seller_name: '滴滴A' }),
      makeInvoice({ amount: 220, category: 'CityTransport', seller_name: '滴滴B' }),
      makeInvoice({ amount: 200, category: 'CityTransport', seller_name: '滴滴C' }),
    ])
    const result = analyzeTripOverage(trip, makeForm({
      travel_days: 5,
      city_transport_actual_amount: 660,
      city_transport_amount: 400, // 80 × 5
    }))
    const ct = result.cityTransport!
    expect(ct.over).toBe(true)
    expect(ct.overAmount).toBeCloseTo(260, 6)
    expect(ct.usageRate).toBeCloseTo((660 / 400) * 100, 6)
    // 最小移交组合：220 + 200 = 420（单张最大 240 < 260，两两组合中 420 最小）
    expect(ct.suggestedInvoices.map(s => s.amount)).toEqual([220, 200])
    expect(ct.suggestedInvoicesTotal).toBe(420)
    expect(result.overTotal).toBeCloseTo(260, 6)
  })

  it('市内交通超标：行程同样按最小移交金额选（140+120 恰好覆盖超额 260）', () => {
    const trip = makeTrip([
      makeInvoice({
        amount: 240, category: 'CityTransport',
        itineraries: [makeRide({ amount: 150, pickup: 'A' }), makeRide({ amount: 90, pickup: 'B' })],
      }),
      makeInvoice({
        amount: 220, category: 'CityTransport', invoice_number: 'INV-B-0000123456',
        itineraries: [makeRide({ amount: 140, pickup: 'C' }), makeRide({ amount: 80, pickup: 'D' })],
      }),
      makeInvoice({
        amount: 200, category: 'CityTransport',
        itineraries: [makeRide({ amount: 120, pickup: 'E' }), makeRide({ amount: 80, pickup: 'F' })],
      }),
    ])
    const result = analyzeTripOverage(trip, makeForm({
      travel_days: 5,
      city_transport_actual_amount: 660,
      city_transport_amount: 400,
    }))
    const rides = result.cityTransport!.suggestedRides
    // 全部行程 [150,140,120,90,80,80] 中合计 ≥260 的最小组合是 140+120=260（精确命中）
    expect(rides.map(r => r.amount)).toEqual([140, 120])
    expect(result.cityTransport!.suggestedRidesTotal).toBe(260)
    // 行程携带所属发票号，供重新开票/换开时定位原发票
    expect(rides.find(r => r.amount === 140)!.invoiceNumber).toBe('INV-B-0000123456')
  })

  it('行程合计不足以覆盖超额时移交全部行程', () => {
    const trip = makeTrip([
      makeInvoice({ amount: 300, category: 'Toll' }),
      makeInvoice({
        amount: 150, category: 'CityTransport',
        itineraries: [makeRide({ amount: 30 }), makeRide({ amount: 15 })],
      }),
    ])
    const result = analyzeTripOverage(trip, makeForm({
      travel_days: 5,
      city_transport_actual_amount: 450,
      city_transport_amount: 400, // 超额 50
    }))
    const ct = result.cityTransport!
    expect(ct.over).toBe(true)
    expect(ct.overAmount).toBeCloseTo(50, 6)
    // 行程合计 45 < 50 → 全部移交
    expect(ct.suggestedRides.map(r => r.amount)).toEqual([30, 15])
    expect(ct.suggestedRidesTotal).toBe(45)
    // 发票最小移交：150（150 ≥ 50，单张最小）
    expect(ct.suggestedInvoices.map(s => s.amount)).toEqual([150])
  })

  it('住宿超标：按发票给出超标金额与平均每晚超标', () => {
    const over1 = makeInvoice({
      amount: 4222.63,
      category: 'Hotel',
      seller_name: '成都某酒店',
      hotel_detail: { nights: 11, nightly_rate: 383.87, check_in: '2025-08-04', check_out: '2025-08-15' },
    })
    const over2 = makeInvoice({
      amount: 1200,
      category: 'Hotel',
      hotel_detail: { nights: 3, nightly_rate: 400, check_in: '2025-08-16', check_out: '2025-08-19' },
    })
    const ok = makeInvoice({ amount: 700, category: 'Hotel', hotel_detail: { nights: 2, nightly_rate: 350, check_in: '2025-08-19', check_out: '2025-08-21' } })
    const trip = makeTrip([over1, over2, ok])
    const result = analyzeTripOverage(trip, makeForm({
      hotel_levels: [{ level: '其他人员', persons: 1, days: 16, daily_rate: 370, amount: 5422.63, actual_amount: 6122.63 }],
    }))
    expect(result.over).toBe(true)
    expect(result.cityTransport).toBeNull()
    const hotel = result.hotel!
    expect(hotel.items).toHaveLength(2)
    expect(hotel.items[0].invoiceId).toBe(over1.id) // 按超标金额降序
    // 4222.63 - 370×11 = 152.63
    expect(hotel.items[0].overAmount).toBeCloseTo(152.63, 2)
    expect(hotel.items[0].perNightOver).toBeCloseTo(152.63 / 11, 6)
    // 1200 - 370×3 = 90
    expect(hotel.items[1].overAmount).toBeCloseTo(90, 6)
    // 未超标的发票不出现
    expect(hotel.items.find(i => i.invoiceId === ok.id)).toBeUndefined()
    expect(hotel.overAmount).toBeCloseTo(242.63, 2)
    expect(hotel.overNights).toBe(14)
    // 使用率只统计住宿类目：6122.63 / (370×16)
    expect(hotel.usageRate).toBeCloseTo((6122.63 / 5920) * 100, 6)
    expect(result.overTotal).toBeCloseTo(242.63, 2)
  })

  it('住宿发票缺少 hotel_detail 时按行程天数估算晚数并标记 estimated', () => {
    const inv = makeInvoice({ amount: 3000, category: 'Hotel' }) // 无 hotel_detail
    const trip = makeTrip([inv])
    const result = analyzeTripOverage(trip, makeForm({
      // travel_days=6 → 估算 5 晚，标准 350×5=1750
      hotel_levels: [{ level: '其他人员', persons: 1, days: 5, daily_rate: 350, amount: 1750, actual_amount: 3000 }],
    }))
    const hotel = result.hotel!
    expect(hotel.items).toHaveLength(1)
    expect(hotel.items[0].estimated).toBe(true)
    expect(hotel.items[0].nights).toBe(5)
    expect(hotel.items[0].overAmount).toBeCloseTo(1250, 6)
  })

  it('只有市内交通有支出时只出现市内交通类目', () => {
    const trip = makeTrip([makeInvoice({ amount: 600, category: 'CityTransport' })])
    const result = analyzeTripOverage(trip, makeForm({
      city_transport_actual_amount: 600,
      city_transport_amount: 480,
      hotel_levels: [],
    }))
    expect(result.hotel).toBeNull()
    expect(result.cityTransport!.usageRate).toBeCloseTo((600 / 480) * 100, 6)
  })

  it('daily_std 缺失（旧数据）时按 80 元/天兜底', () => {
    const trip = makeTrip([makeInvoice({ amount: 600, category: 'CityTransport' })])
    const result = analyzeTripOverage(trip, makeForm({
      city_transport_daily_std: 0,
      city_transport_actual_amount: 600,
      city_transport_amount: 480,
    }))
    expect(result.cityTransport!.dailyStd).toBe(80)
    expect(result.over).toBe(true)
  })
})
