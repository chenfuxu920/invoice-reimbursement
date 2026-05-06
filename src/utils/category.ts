import type { InvoiceCategory } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'

export interface CategoryStyle {
  label: string
  icon: string
  bgColor: string
  textColor: string
}

const CATEGORY_STYLES: Record<InvoiceCategory, CategoryStyle> = {
  Train: { label: CATEGORY_LABELS.Train, icon: '🚄', bgColor: 'bg-green-100', textColor: 'text-green-700' },
  Flight: { label: CATEGORY_LABELS.Flight, icon: '✈️', bgColor: 'bg-blue-100', textColor: 'text-blue-700' },
  TicketChange: { label: CATEGORY_LABELS.TicketChange, icon: '🔄', bgColor: 'bg-orange-100', textColor: 'text-orange-700' },
  CityTransport: { label: CATEGORY_LABELS.CityTransport, icon: '🚕', bgColor: 'bg-purple-100', textColor: 'text-purple-700' },
  Hotel: { label: CATEGORY_LABELS.Hotel, icon: '🏨', bgColor: 'bg-yellow-100', textColor: 'text-yellow-700' },
  Meal: { label: CATEGORY_LABELS.Meal, icon: '🍜', bgColor: 'bg-red-100', textColor: 'text-red-700' },
  Other: { label: CATEGORY_LABELS.Other, icon: '📋', bgColor: 'bg-gray-100', textColor: 'text-gray-700' },
}

export function getCategoryStyle(category: InvoiceCategory): CategoryStyle {
  return CATEGORY_STYLES[category] || CATEGORY_STYLES.Other
}

export function getCategoryLabel(category: InvoiceCategory): string {
  return getCategoryStyle(category).label
}

export function getCategoryIcon(category: InvoiceCategory): string {
  return getCategoryStyle(category).icon
}

export function getCategoryBadgeClass(category: InvoiceCategory): string {
  const style = getCategoryStyle(category)
  return `${style.bgColor} ${style.textColor}`
}
