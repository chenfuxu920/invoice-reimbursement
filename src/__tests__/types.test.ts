import { describe, it, expect } from 'vitest'
import type {
  InvoiceCategory,
  InvoiceSource,
  Itinerary,
  Invoice,
  PaymentSource,
  PaymentRecord,
  MatchType,
  MatchResult,
  CategorySummary,
  ReimbursementForm,
} from '../types'

describe('Type definitions integrity', () => {
  it('InvoiceCategory should cover all expected values', () => {
    const categories: InvoiceCategory[] = [
      'Train',
      'Flight',
      'TicketChange',
      'CityTransport',
      'Hotel',
      'Meal',
      'Toll',
      'Other',
    ]
    expect(categories).toHaveLength(8)
    expect(new Set(categories).size).toBe(8)
  })

  it('PaymentSource should cover Wechat and Alipay', () => {
    const sources: PaymentSource[] = ['Wechat', 'Alipay']
    expect(sources).toHaveLength(2)
  })

  it('MatchType should cover all expected values', () => {
    const matchTypes: MatchType[] = [
      'OneToOne',
      'OneToMany',
      'Unmatched',
      'ManualConfirmed',
    ]
    expect(matchTypes).toHaveLength(4)
    expect(new Set(matchTypes).size).toBe(4)
  })

  it('InvoiceSource should have type and path', () => {
    const photoSource: InvoiceSource = { type: 'Photo', path: '/img/test.jpg' }
    const pdfSource: InvoiceSource = { type: 'Pdf', path: '/doc/test.pdf' }
    const linkSource: InvoiceSource = { type: 'Link', path: 'http://example.com/inv' }

    expect(photoSource.type).toBe('Photo')
    expect(pdfSource.type).toBe('Pdf')
    expect(linkSource.type).toBe('Link')
  })

  it('Itinerary should have all required fields', () => {
    const itinerary: Itinerary = {
      date_time: '2025-01-15 09:00',
      provider: '滴滴',
      pickup: '北京站',
      dropoff: '国贸',
      amount: 35.0,
    }
    expect(itinerary.date_time).toBeTruthy()
    expect(itinerary.provider).toBeTruthy()
    expect(itinerary.amount).toBeGreaterThan(0)
  })

  it('Invoice should have all required fields', () => {
    const invoice: Invoice = {
      id: 'inv1',
      invoice_number: 'INV001',
      amount: 100.0,
      seller_name: '测试酒店',
      item_name: '住宿费',
      date: '2025-01-15',
      category: 'Hotel',
      source: { type: 'Photo', path: '/img/hotel.jpg' },
      itineraries: [],
    }
    expect(invoice.id).toBeTruthy()
    expect(invoice.amount).toBeGreaterThan(0)
    expect(invoice.category).toBe('Hotel')
    expect(invoice.itineraries).toEqual([])
  })

  it('PaymentRecord should have all required fields', () => {
    const payment: PaymentRecord = {
      id: 'pay1',
      transaction_id: 'TX001',
      transaction_time: '2025-01-15 12:00',
      amount: 100.0,
      original_amount: 100.0,
      refund_amount: 0,
      discount: 0,
      merchant_name: '测试酒店',
      source: 'Wechat',
      category: '住宿',
      payment_method: '',
    }
    expect(payment.id).toBeTruthy()
    expect(payment.amount).toBeGreaterThan(0)
    expect(payment.source).toBe('Wechat')
  })

  it('MatchResult should have all required fields', () => {
    const matchResult: MatchResult = {
      invoice_id: 'inv1',
      invoice: {
        id: 'inv1',
        invoice_number: 'INV001',
        amount: 100.0,
        seller_name: '测试酒店',
        item_name: '住宿费',
        date: '2025-01-15',
        category: 'Hotel',
        source: { type: 'Photo', path: '/img/hotel.jpg' },
        itineraries: [],
      },
      payment_ids: ['pay1'],
      payments: [
        {
          id: 'pay1',
          transaction_id: 'TX001',
          transaction_time: '2025-01-15 12:00',
          amount: 100.0,
          original_amount: 100.0,
          refund_amount: 0.0,
          discount: 0.0,
          merchant_name: '测试酒店',
          source: 'Wechat',
          category: '住宿',
          payment_method: '',
        },
      ],
      match_type: 'OneToOne',
      confidence: 1.0,
      amount_diff: 0.0,
    }
    expect(matchResult.invoice_id).toBe('inv1')
    expect(matchResult.payment_ids).toHaveLength(1)
    expect(matchResult.match_type).toBe('OneToOne')
    expect(matchResult.confidence).toBeGreaterThanOrEqual(0)
    expect(matchResult.confidence).toBeLessThanOrEqual(1)
  })

  it('CategorySummary should have category, count, and total_amount', () => {
    const summary: CategorySummary = {
      category: 'Hotel',
      count: 2,
      total_amount: 900.0,
    }
    expect(summary.count).toBe(2)
    expect(summary.total_amount).toBeGreaterThan(0)
  })

  it('ReimbursementForm should have all required fields', () => {
    const form: ReimbursementForm = {
      name: '张三',
      department: '技术部',
      destination: '上海',
      travel_start: '2025-01-15',
      travel_end: '2025-01-20',
      travel_days: 6,
      companions: 2,
      transport_details: [
        { label: '车、船票', count: 1, amount: 553.0 },
      ],
      transport_subtotal: 553.0,
      city_transport_count: 5,
      city_transport_amount: 200.0,
      city_transport_actual_amount: 200.0,
      hotel_levels: [
        { level: '其他人员', persons: 1, days: 5, daily_rate: 180.0, amount: 900.0, actual_amount: 900.0 },
      ],
      hotel_subtotal: 900.0,
      meal_subsidy: { persons: 1, days: 6, daily_rate: 100.0, amount: 600.0 },
      baggage_amount: 0.0,
      meal_reimbursement: 0.0,
      advance_payment: 0.0,
      summaries: [
        { category: 'Train', count: 1, total_amount: 553.0 },
        { category: 'Hotel', count: 2, total_amount: 900.0 },
      ],
      total_amount: 2253.0,
    }
    expect(form.name).toBeTruthy()
    expect(form.summaries.length).toBeGreaterThan(0)
    expect(form.total_amount).toBeGreaterThan(0)
    expect(form.transport_details.length).toBeGreaterThan(0)
    expect(form.meal_subsidy.amount).toBeGreaterThan(0)
  })

  it('Invoice with itineraries should be valid', () => {
    const invoice: Invoice = {
      id: 'inv-taxi',
      invoice_number: 'INV-TAXI001',
      amount: 100.0,
      seller_name: '滴滴出行',
      item_name: '市内交通',
      date: '2025-01-15',
      category: 'CityTransport',
      source: { type: 'Photo', path: '/img/taxi.jpg' },
      itineraries: [
        {
          date_time: '2025-01-15 09:00',
          provider: '滴滴',
          pickup: '北京站',
          dropoff: '国贸',
          amount: 30.0,
        },
        {
          date_time: '2025-01-15 18:00',
          provider: '高德',
          pickup: '国贸',
          dropoff: '北京站',
          amount: 70.0,
        },
      ],
    }
    expect(invoice.category).toBe('CityTransport')
    expect(invoice.itineraries).toHaveLength(2)
    const totalItineraryAmount = invoice.itineraries.reduce((sum, it) => sum + it.amount, 0)
    expect(totalItineraryAmount).toBe(100.0)
  })
})
