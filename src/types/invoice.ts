export type InvoiceCategory =
  | 'Train'
  | 'Flight'
  | 'Insurance'
  | 'TicketChange'
  | 'CityTransport'
  | 'Hotel'
  | 'Meal'
  | 'Toll'
  | 'Other'

export interface InvoiceSource {
  type: 'Photo' | 'Pdf' | 'Link' | 'Manual'
  path?: string
}

/// 住宿发票详情（Rust HotelDetail 序列化；仅住宿类发票有值）
export interface HotelDetail {
  check_in?: string | null
  check_out?: string | null
  nights: number
  nightly_rate: number
}

export interface Itinerary {
  date_time: string
  provider: string
  pickup: string
  dropoff: string
  amount: number
  city?: string
  incomplete_fields: string[]
}

export interface Invoice {
  id: string
  invoice_number: string
  amount: number
  seller_name: string
  item_name: string
  date: string
  travel_date?: string
  /// 出发时刻 HH:MM（仅 Train/Flight；从票面"日期 时间"提取）
  travel_time?: string | null
  category: InvoiceCategory
  source: InvoiceSource
  itineraries: Itinerary[]
  itinerary_file?: string | null
  remarks?: string
  hotel_detail?: HotelDetail | null
  // NEW
  departure_city?: string
  arrival_city?: string
  /// 通行时间（仅 Toll 类发票有值；Rust NaiveDateTime 序列化为 "YYYY-MM-DDTHH:MM:SS"）
  toll_travel_time?: string | null
}

export const CATEGORY_LABELS: Record<InvoiceCategory, string> = {
  Train: '高铁/车船票',
  Flight: '飞机票',
  Insurance: '保险费',
  TicketChange: '退改签',
  CityTransport: '市内交通',
  Hotel: '住宿费',
  Meal: '餐饮费',
  Toll: '高速通行费',
  Other: '其他',
}

/// 解析失败的文件条目，用于导入界面错误区展示与手动填写入口
export interface ParseError {
  id: string
  filePath: string
  fileName: string
  message: string
}
