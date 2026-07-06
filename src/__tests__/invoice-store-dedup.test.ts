import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock @tauri-apps/api/core 的 invoke，避免真实 Tauri 调用
const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))

// createPinia 必须在导入 store 之前可用
import { createPinia, setActivePinia } from 'pinia'
import { useInvoiceStore } from '../stores/invoice'
import type { Invoice } from '../types'

function makeInvoice(id: string, number: string): Invoice {
  return {
    id,
    invoice_number: number,
    amount: 100.0,
    seller_name: '测试销售方',
    item_name: '测试项目',
    date: '2025-01-15',
    category: 'Hotel',
    source: { type: 'Photo', path: `/img/${id}.jpg` },
    itineraries: [],
  }
}

describe('invoiceStore 跨批次去重', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invokeMock.mockReset()
  })

  it('addInvoicesSkipDuplicates 跳过与已有列表发票号重复的项', () => {
    const store = useInvoiceStore()
    // 预置一张已存在的发票
    store.invoices.push(makeInvoice('exist', 'INV001'))

    const skipped = store.addInvoicesSkipDuplicates([
      makeInvoice('a', 'INV001'), // 重复，应跳过
      makeInvoice('b', 'INV002'), // 新增
      makeInvoice('c', 'INV003'), // 新增
    ])

    expect(skipped).toEqual(['INV001'])
    expect(store.invoices).toHaveLength(3)
    expect(store.invoices.map(i => i.invoice_number)).toEqual(['INV001', 'INV002', 'INV003'])
  })

  it('无发票号的项不被跨批次去重（与后端 dedup 语义一致）', () => {
    const store = useInvoiceStore()
    store.invoices.push(makeInvoice('exist', ''))

    const skipped = store.addInvoicesSkipDuplicates([
      makeInvoice('a', ''), // 无号，放行
    ])

    expect(skipped).toEqual([])
    expect(store.invoices).toHaveLength(2)
  })

  it('addInvoicesSkipDuplicates 对空列表返回空且不改动', () => {
    const store = useInvoiceStore()
    const skipped = store.addInvoicesSkipDuplicates([])
    expect(skipped).toEqual([])
    expect(store.invoices).toHaveLength(0)
  })

  it('addInvoice 跨批次去重：重复时返回 false 且不追加', async () => {
    const store = useInvoiceStore()
    store.invoices.push(makeInvoice('exist', 'INV001'))

    invokeMock.mockResolvedValue(makeInvoice('new', 'INV001'))

    const added = await store.addInvoice('/img/new.jpg', 'image')

    expect(added).toBe(false)
    expect(store.invoices).toHaveLength(1)
    expect(store.invoices[0].id).toBe('exist')
  })

  it('addInvoice 无重复时返回 true 并追加', async () => {
    const store = useInvoiceStore()
    invokeMock.mockResolvedValue(makeInvoice('new', 'INV999'))

    const added = await store.addInvoice('/img/new.jpg', 'image')

    expect(added).toBe(true)
    expect(store.invoices).toHaveLength(1)
    expect(store.invoices[0].invoice_number).toBe('INV999')
  })

  it('addInvoice 无发票号时不判重，直接追加', async () => {
    const store = useInvoiceStore()
    store.invoices.push(makeInvoice('exist', ''))
    invokeMock.mockResolvedValue(makeInvoice('new', ''))

    const added = await store.addInvoice('/img/new.jpg', 'image')

    expect(added).toBe(true)
    expect(store.invoices).toHaveLength(2)
  })
})
