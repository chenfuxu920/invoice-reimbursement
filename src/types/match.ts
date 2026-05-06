import type { Invoice } from './invoice'
import type { PaymentRecord } from './payment'

export type MatchType = 'OneToOne' | 'OneToMany' | 'Unmatched' | 'ManualConfirmed'

export interface MatchResult {
  invoice_id: string
  invoice: Invoice
  payment_ids: string[]
  payments: PaymentRecord[]
  match_type: MatchType
  confidence: number
  amount_diff: number
}
