import { describe, it, expect, vi, beforeEach } from 'vitest'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))

import { createPinia, setActivePinia } from 'pinia'
import { useInvoiceStore } from '../stores/invoice'
import type { Invoice, ParseError } from '../types'

function makeInvoice(id: string): Invoice {
  return {
    id,
    invoice_number: 'INV' + id,
    amount: 100.0,
    seller_name: '测试销售方',
    item_name: '测试项目',
    date: '2025-01-15',
    category: 'Hotel',
    source: { type: 'Photo', path: `/img/${id}.jpg` },
    itineraries: [],
  }
}

function makeError(id: string): ParseError {
  return { id, filePath: `/err/${id}.pdf`, fileName: `${id}.pdf`, message: '解析失败' }
}

describe('invoiceStore 解析错误状态', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invokeMock.mockReset()
  })

  it('addParseErrors 批量追加错误到 parseErrors', () => {
    const store = useInvoiceStore()
    store.addParseErrors([makeError('e1'), makeError('e2')])
    expect(store.parseErrors).toHaveLength(2)
    expect(store.parseErrors.map(e => e.id)).toEqual(['e1', 'e2'])
  })

  it('addParseErrors 在已有错误基础上追加（不覆盖）', () => {
    const store = useInvoiceStore()
    store.addParseErrors([makeError('e1')])
    store.addParseErrors([makeError('e2')])
    expect(store.parseErrors).toHaveLength(2)
  })

  it('removeParseError 按 id 移除单条错误', () => {
    const store = useInvoiceStore()
    store.addParseErrors([makeError('e1'), makeError('e2'), makeError('e3')])
    store.removeParseError('e2')
    expect(store.parseErrors).toHaveLength(2)
    expect(store.parseErrors.map(e => e.id)).toEqual(['e1', 'e3'])
  })

  it('clearParseErrors 清空全部错误', () => {
    const store = useInvoiceStore()
    store.addParseErrors([makeError('e1'), makeError('e2')])
    store.clearParseErrors()
    expect(store.parseErrors).toHaveLength(0)
  })

  it('clearInvoices 同时清空 parseErrors', () => {
    const store = useInvoiceStore()
    store.invoices.push(makeInvoice('inv1'))
    store.addParseErrors([makeError('e1')])
    store.clearInvoices()
    expect(store.invoices).toHaveLength(0)
    expect(store.parseErrors).toHaveLength(0)
  })

  it('addManualInvoice 将发票加入 invoices 列表', () => {
    const store = useInvoiceStore()
    const inv = makeInvoice('m1')
    store.addManualInvoice(inv)
    expect(store.invoices).toHaveLength(1)
    expect(store.invoices[0].id).toBe('m1')
  })

  it('addManualInvoice 后 removeParseError 配合使用：手动填写保存后错误被移除', () => {
    const store = useInvoiceStore()
    store.addParseErrors([makeError('e1')])
    store.addManualInvoice(makeInvoice('m1'))
    store.removeParseError('e1')
    expect(store.invoices).toHaveLength(1)
    expect(store.parseErrors).toHaveLength(0)
  })
})
