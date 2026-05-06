import type { InvoiceCategory } from './invoice'

export interface CategorySummary {
  category: InvoiceCategory
  count: number
  total_amount: number
}

export interface ReimbursementForm {
  name: string
  department: string
  travel_start: string
  travel_end: string
  companions: number
  summaries: CategorySummary[]
  total_amount: number
}
