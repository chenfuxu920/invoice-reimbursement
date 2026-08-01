import { describe, it, expect, vi, beforeEach } from 'vitest'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))

import { createPinia, setActivePinia } from 'pinia'
import { useMatchStore } from '../stores/match'
import type { Invoice, MatchResult } from '../types'

function makeInvoice(id: string, opts: Partial<Invoice> = {}): Invoice {
  return {
    id,
    invoice_number: '',
    amount: 100,
    seller_name: '',
    item_name: '',
    date: '2026-05-20',
    category: 'Train',
    source: { type: 'Manual' },
    itineraries: [],
    ...opts,
  }
}

function makeMatch(id: string, opts: Partial<Invoice> = {}): MatchResult {
  const invoice = makeInvoice(id, opts)
  return {
    invoice_id: id,
    invoice,
    payment_ids: [],
    payments: [],
    match_type: 'OneToOne',
    confidence: 1,
    amount_diff: 0,
    itinerary_payment_pairs: [],
  }
}

describe('matchStore 分趟逻辑', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invokeMock.mockReset()
  })

  it('resegment 映射 trips 与 unassigned', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({
      trips: [{
        id: 'trip-1', destination: '上海', travel_start: '2026-05-20', travel_end: '2026-05-22',
        ticket_ids: ['t1', 't2'], invoice_ids: ['t1', 't2'],
      }],
      unassigned_ids: ['m1'],
    })
    const matches = [
      makeMatch('t1', { departure_city: '长沙', arrival_city: '上海', travel_date: '2026-05-20' }),
      makeMatch('t2', { departure_city: '上海', arrival_city: '长沙', travel_date: '2026-05-22' }),
      makeMatch('m1', { category: 'Meal' }),
    ]
    await store.resegment(matches, '')
    expect(store.trips).toHaveLength(1)
    expect(store.trips[0].destination).toBe('上海')
    expect(store.trips[0].travelStart).toBe('2026-05-20')
    expect(store.trips[0].travelEnd).toBe('2026-05-22')
    expect(store.trips[0].ticketIds).toEqual(['t1', 't2'])
    expect(store.trips[0].matches.map(m => m.invoice_id)).toEqual(['t1', 't2'])
    expect(store.unassigned.map(m => m.invoice_id)).toEqual(['m1'])
    expect(invokeMock).toHaveBeenCalledWith('segment_trips', {
      matchResults: matches,
      origin: null,
    })
  })

  it('resegment 携带 origin', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({ trips: [], unassigned_ids: ['t1'] })
    await store.resegment([makeMatch('t1')], '长沙')
    expect(invokeMock).toHaveBeenCalledWith('segment_trips', {
      matchResults: expect.anything(),
      origin: '长沙',
    })
  })

  it('无票据时兜底为单趟', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({ trips: [], unassigned_ids: ['m1', 'm2'] })
    const matches = [
      makeMatch('m1', { category: 'Meal' }),
      makeMatch('m2', { category: 'Hotel' }),
    ]
    await store.resegment(matches, '')
    expect(store.trips).toHaveLength(1)
    expect(store.trips[0].matches).toHaveLength(2)
    expect(store.unassigned).toHaveLength(0)
  })

  it('moveToTrip 移到另一趟/待调整', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({
      trips: [
        { id: 'trip-1', destination: '上海', travel_start: '2026-05-20', travel_end: '2026-05-22', ticket_ids: ['t1'], invoice_ids: ['t1'] },
        { id: 'trip-2', destination: '成都', travel_start: '2026-06-01', travel_end: '2026-06-03', ticket_ids: ['t2'], invoice_ids: ['t2'] },
      ],
      unassigned_ids: [],
    })
    const matches = [makeMatch('t1'), makeMatch('t2')]
    await store.resegment(matches, '')

    store.moveToTrip('t1', 'trip-2')
    expect(store.trips[0].matches).toHaveLength(0)
    expect(store.trips[1].matches.map(m => m.invoice_id)).toEqual(['t2', 't1'])

    store.moveToTrip('t1', null)
    expect(store.unassigned.map(m => m.invoice_id)).toEqual(['t1'])

    store.moveToTrip('t1', 'trip-1')
    expect(store.trips[0].matches.map(m => m.invoice_id)).toEqual(['t1'])
    expect(store.unassigned).toHaveLength(0)
  })

  it('moveToTrip 同步票据 ticketIds', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({
      trips: [
        { id: 'trip-1', destination: '上海', travel_start: '2026-05-20', travel_end: '2026-05-22', ticket_ids: ['t1'], invoice_ids: ['t1'] },
        { id: 'trip-2', destination: '成都', travel_start: '2026-06-01', travel_end: '2026-06-03', ticket_ids: ['t2'], invoice_ids: ['t2'] },
      ],
      unassigned_ids: [],
    })
    const matches = [makeMatch('t1'), makeMatch('t2')]
    await store.resegment(matches, '')

    store.moveToTrip('t1', 'trip-2')
    expect(store.trips[0].ticketIds).toEqual([])
    expect(store.trips[1].ticketIds).toEqual(['t2', 't1'])

    store.moveToTrip('t1', null)
    expect(store.trips[1].ticketIds).toEqual(['t2'])
    expect(store.unassigned.map(m => m.invoice_id)).toEqual(['t1'])

    store.moveToTrip('t1', 'trip-1')
    expect(store.trips[0].ticketIds).toEqual(['t1'])
    expect(store.unassigned).toHaveLength(0)
  })

  it('moveToTrip 非票据不改变 ticketIds', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({
      trips: [
        { id: 'trip-1', destination: '上海', travel_start: '2026-05-20', travel_end: '2026-05-22', ticket_ids: ['t1'], invoice_ids: ['t1', 'm1'] },
      ],
      unassigned_ids: [],
    })
    const matches = [makeMatch('t1'), makeMatch('m1', { category: 'Meal' })]
    await store.resegment(matches, '')

    store.moveToTrip('m1', null)
    expect(store.trips[0].ticketIds).toEqual(['t1'])
    expect(store.unassigned.map(m => m.invoice_id)).toEqual(['m1'])
  })

  it('createTripFromTicket 从待调整票据新建出差', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({ trips: [], unassigned_ids: ['t1'] })
    const ticket = makeMatch('t1', {
      departure_city: '长沙', arrival_city: '武汉', travel_date: '2026-05-20',
    })
    await store.resegment([ticket], '')
    store.createTripFromTicket(ticket)
    expect(store.trips).toHaveLength(1)
    expect(store.trips[0].destination).toBe('武汉')
    expect(store.trips[0].travelStart).toBe('2026-05-20')
    expect(store.trips[0].travelEnd).toBe('2026-05-20')
    expect(store.trips[0].ticketIds).toEqual(['t1'])
    expect(store.unassigned).toHaveLength(0)
  })

  it('createTripFromTicket 守卫：已在 trip 中的票据不重复归入', async () => {
    const store = useMatchStore()
    invokeMock.mockResolvedValue({
      trips: [
        { id: 'trip-1', destination: '上海', travel_start: '2026-05-20', travel_end: '2026-05-22', ticket_ids: ['t1'], invoice_ids: ['t1'] },
        { id: 'trip-2', destination: '成都', travel_start: '2026-06-01', travel_end: '2026-06-03', ticket_ids: ['t2'], invoice_ids: ['t2'] },
      ],
      unassigned_ids: [],
    })
    const matches = [makeMatch('t1'), makeMatch('t2')]
    await store.resegment(matches, '')

    store.moveToTrip('t1', 'trip-2')
    store.createTripFromTicket(store.trips[1].matches[0])

    expect(store.trips).toHaveLength(2)
    expect(store.trips[1].matches.map(m => m.invoice_id)).toEqual(['t2', 't1'])
    expect(store.unassigned).toHaveLength(0)
  })

  it('updateMatchInvoice 就地更新匹配中的发票（趟内同步）', () => {
    const store = useMatchStore()
    const m = makeMatch('t1', { category: 'Hotel' })
    store.matches.push(m)
    store.trips.push({
      id: 'trip-1', destination: '上海', travelStart: '2026-05-20', travelEnd: '2026-05-22',
      hotelLevel: '其他人员', ticketIds: ['t1'], matches: [m],
    })

    store.updateMatchInvoice({ ...m.invoice, amount: 999 })

    expect(store.matches[0].invoice.amount).toBe(999)
    expect(store.trips[0].matches[0].invoice.amount).toBe(999)
  })
})
