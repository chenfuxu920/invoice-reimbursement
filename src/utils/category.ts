import type { InvoiceCategory } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import type { IconName } from '../components/ui/AppIcon.vue'

export interface CategoryStyle {
  label: string
  icon: IconName
  badgeClass: string
}

const CATEGORY_STYLES: Record<InvoiceCategory, CategoryStyle> = {
  Train: { label: CATEGORY_LABELS.Train, icon: 'train', badgeClass: 'bg-emerald-50 text-emerald-700' },
  Flight: { label: CATEGORY_LABELS.Flight, icon: 'plane', badgeClass: 'bg-primary-50 text-primary-700' },
  Insurance: { label: CATEGORY_LABELS.Insurance, icon: 'shield', badgeClass: 'bg-cyan-50 text-cyan-700' },
  TicketChange: { label: CATEGORY_LABELS.TicketChange, icon: 'swap', badgeClass: 'bg-amber-50 text-amber-700' },
  CityTransport: { label: CATEGORY_LABELS.CityTransport, icon: 'car', badgeClass: 'bg-purple-50 text-purple-700' },
  Hotel: { label: CATEGORY_LABELS.Hotel, icon: 'hotel', badgeClass: 'bg-yellow-50 text-yellow-700' },
  Meal: { label: CATEGORY_LABELS.Meal, icon: 'meal', badgeClass: 'bg-rose-50 text-rose-700' },
  Toll: { label: CATEGORY_LABELS.Toll, icon: 'toll', badgeClass: 'bg-indigo-50 text-indigo-700' },
  Other: { label: CATEGORY_LABELS.Other, icon: 'clipboard', badgeClass: 'bg-gray-100 text-gray-700' },
}

export function getCategoryStyle(category: InvoiceCategory): CategoryStyle {
  return CATEGORY_STYLES[category] || CATEGORY_STYLES.Other
}

export function getCategoryLabel(category: InvoiceCategory): string {
  return getCategoryStyle(category).label
}

export function getCategoryIcon(category: InvoiceCategory): IconName {
  return getCategoryStyle(category).icon
}

export function getCategoryBadgeClass(category: InvoiceCategory): string {
  return getCategoryStyle(category).badgeClass
}
