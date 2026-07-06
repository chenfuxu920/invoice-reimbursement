import type { InvoiceCategory } from './invoice'

export interface CategorySummary {
  category: InvoiceCategory
  count: number
  total_amount: number
}

export interface TransportDetail {
  label: string
  count: number
  amount: number
}

export interface HotelLevelDetail {
  level: string
  persons: number
  days: number
  daily_rate: number
  amount: number
  actual_amount: number
}

export interface MealSubsidyDetail {
  persons: number
  days: number
  daily_rate: number
  amount: number
}

export interface ReimbursementForm {
  name: string
  department: string
  destination: string
  travel_start: string
  travel_end: string
  travel_days: number
  companions: number
  transport_details: TransportDetail[]
  transport_subtotal: number
  city_transport_count: number
  city_transport_amount: number
  city_transport_actual_amount: number
  hotel_levels: HotelLevelDetail[]
  hotel_subtotal: number
  meal_subsidy: MealSubsidyDetail
  baggage_amount: number
  meal_reimbursement: number
  summaries: CategorySummary[]
  total_amount: number
}
