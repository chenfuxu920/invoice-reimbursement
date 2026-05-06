export type InvoiceCategory =
  | 'Train'
  | 'Flight'
  | 'TicketChange'
  | 'CityTransport'
  | 'Hotel'
  | 'Meal'
  | 'Other'

export interface InvoiceSource {
  type: 'Photo' | 'Pdf' | 'Link'
  path: string
}

export interface Itinerary {
  date_time: string
  provider: string
  pickup: string
  dropoff: string
  amount: number
}

export interface Invoice {
  id: string
  invoice_number: string
  amount: number
  seller_name: string
  item_name: string
  date: string
  category: InvoiceCategory
  source: InvoiceSource
  itineraries: Itinerary[]
}

export const CATEGORY_LABELS: Record<InvoiceCategory, string> = {
  Train: '高铁/车船票',
  Flight: '飞机票',
  TicketChange: '退改签/保险费',
  CityTransport: '市内交通',
  Hotel: '住宿费',
  Meal: '餐饮费',
  Other: '其他',
}
