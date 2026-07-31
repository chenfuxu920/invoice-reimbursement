import type { Invoice } from './invoice'
import type { PaymentRecord } from './payment'

export type MatchType = 'OneToOne' | 'OneToMany' | 'Unmatched' | 'ManualConfirmed'

/// 行程条目与支付记录的显式配对（市内交通一对多场景）。
/// itinerary_index 对应 invoice.itineraries 下标，payment_id 对应 payments 中的支付 id。
export interface ItineraryPaymentPair {
  itinerary_index: number
  payment_id: string
}

export interface MatchResult {
  invoice_id: string
  invoice: Invoice
  payment_ids: string[]
  payments: PaymentRecord[]
  match_type: MatchType
  confidence: number
  amount_diff: number
  /// 行程-支付显式配对。非行程场景或旧数据为空。
  itinerary_payment_pairs?: ItineraryPaymentPair[]
}

/// 一趟出差分组：destination/travelStart/travelEnd 预填自票据，用户可手动修改
export interface Trip {
  id: string
  destination: string
  travelStart: string
  travelEnd: string
  hotelLevel: string
  ticketIds: string[]
  matches: MatchResult[]
}
