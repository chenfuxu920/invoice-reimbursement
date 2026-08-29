/**
 * 出差行程费用超标分析（纯函数，供 TripCard 圆环图与超标分析面板使用）。
 *
 * 口径与后端 build_reimbursement_form 一致：
 * - 市内交通费（含高速通行费 Toll）：按「每日标准 × 出差天数」总额封顶；
 * - 住宿费：按目的地每晚标准 × 住宿晚数封顶；
 * - 其余类别实报实销，不存在超标。
 * 标准使用率按类目分开计算（各自实际支出 ÷ 各自标准额度）。
 *
 * 分摊建议逻辑：封顶按总额计算，任选合计不低于超额的发票（或行程）
 * 交由他人报销即可；按「移交金额最小」给出精确解组合（金额转分做子集和 DP）。
 */
import type { Invoice, Trip } from '../types'
import type { ReimbursementForm } from '../types/reimbursement'

const EPS = 0.005 // 金额比较容差（半分）

export interface SuggestedInvoice {
  invoiceId: string
  label: string // 卖方名 → 票号 → id 兜底
  date: string
  amount: number
}

export interface SuggestedRide {
  invoiceId: string
  invoiceLabel: string
  /** 行程所属发票的发票号（重新开票/换开时定位原发票用） */
  invoiceNumber: string
  dateTime: string
  pickup: string
  dropoff: string
  amount: number
}

export interface HotelOverageItem {
  invoiceId: string
  label: string
  dateRange: string
  nights: number
  /** 晚数是否为按行程天数的估算（发票缺少住宿详情时） */
  estimated: boolean
  actual: number
  standard: number
  overAmount: number
  perNightOver: number
}

export interface CityTransportOverage {
  /** 标准使用率（实际 ÷ 封顶额度 %），标准无效时为 null */
  usageRate: number | null
  /** 是否超标 */
  over: boolean
  overAmount: number
  /** 封顶额度 = 每日标准 × 出差天数 */
  standard: number
  actual: number
  dailyStd: number
  days: number
  /** 移交金额最小的发票组合（未超标时为空） */
  suggestedInvoices: SuggestedInvoice[]
  suggestedInvoicesTotal: number
  /** 移交金额最小的行程组合（未超标时为空） */
  suggestedRides: SuggestedRide[]
  suggestedRidesTotal: number
}

export interface HotelOverage {
  /** 标准使用率（实际 ÷ 标准额度 %），标准无效时为 null */
  usageRate: number | null
  over: boolean
  overAmount: number
  dailyRate: number
  standard: number
  actual: number
  /** 超标发票的晚数合计（用于平均每晚超标） */
  overNights: number
  items: HotelOverageItem[]
}

export interface TripOverageAnalysis {
  over: boolean
  overTotal: number
  cityTransport: CityTransportOverage | null
  hotel: HotelOverage | null
}

function invoiceLabel(inv: Invoice): string {
  return inv.seller_name || inv.invoice_number || inv.id
}

function invoiceDate(inv: Invoice): string {
  return inv.travel_date || inv.date
}

/**
 * 从 items 中选出「合计 ≥ target 且合计最小」的组合（移交金额最小）。
 * 金额转分做子集和 DP（BigInt 位集），精确解；无解（合计不足 target）时返回全部。
 * 返回按金额降序排列。
 */
function minTransferPick<T>(items: T[], amount: (t: T) => number, target: number): T[] {
  if (items.length === 0) return []
  const cents = items.map(t => Math.round(amount(t) * 100))
  const targetC = Math.max(Math.round(target * 100), 1)

  // 位集 bit v = 1 表示「存在子集合计恰为 v 分」；bit 0 = 空集
  const snaps: bigint[] = new Array(items.length + 1)
  let bits = 1n
  snaps[0] = bits
  for (let i = 0; i < cents.length; i++) {
    bits |= bits << BigInt(cents[i])
    snaps[i + 1] = bits
  }

  // 找 ≥ targetC 的最小可达和
  const masked = bits >> BigInt(targetC)
  if (masked === 0n) return [...items].sort((a, b) => amount(b) - amount(a)) // 全部加起来都不够 → 移交全部
  const low = masked & -masked
  let cur = targetC + low.toString(2).length - 1

  // 从后向前重建：bit cur 可由前 i 项凑出则跳过第 i 项，否则必选
  const picked: T[] = []
  for (let i = items.length - 1; i >= 0 && cur > 0; i--) {
    const prev = snaps[i]
    if (((prev >> BigInt(cur)) & 1n) === 1n) continue
    picked.push(items[i])
    cur -= cents[i]
  }
  return picked.sort((a, b) => amount(b) - amount(a))
}

function sumBy<T>(items: T[], amount: (t: T) => number): number {
  return items.reduce((s, item) => s + amount(item), 0)
}

export function analyzeTripOverage(trip: Trip, form: ReimbursementForm): TripOverageAnalysis {
  const days = Math.max(form.travel_days, 1)
  const dailyStd = form.city_transport_daily_std > 0 ? form.city_transport_daily_std : 80
  const ctStandard = dailyStd * days
  const ctActual = form.city_transport_actual_amount

  const hotelLevel = form.hotel_levels[0]
  const hotelDailyRate = hotelLevel?.daily_rate ?? 0
  const hotelNights = hotelLevel?.days ?? 0
  const hotelActual = hotelLevel?.actual_amount ?? 0
  const hotelStandard = hotelDailyRate * hotelNights

  // ── 市内交通（有支出才进入分析） ────────────────────────────
  let cityTransport: CityTransportOverage | null = null
  if (ctActual > EPS) {
    const ctOverRaw = ctActual - ctStandard
    const ctOver = ctOverRaw > EPS
    const candidates = trip.matches.filter(
      m => m.invoice.category === 'CityTransport' || m.invoice.category === 'Toll',
    )
    let suggestedInvoices: SuggestedInvoice[] = []
    let suggestedRides: SuggestedRide[] = []
    if (ctOver) {
      suggestedInvoices = minTransferPick(candidates, m => m.invoice.amount, ctOverRaw)
        .map(m => ({
          invoiceId: m.invoice_id,
          label: invoiceLabel(m.invoice),
          date: invoiceDate(m.invoice),
          amount: m.invoice.amount,
        }))
      const rides: SuggestedRide[] = []
      for (const m of candidates) {
        for (const it of m.invoice.itineraries ?? []) {
          rides.push({
            invoiceId: m.invoice_id,
            invoiceLabel: invoiceLabel(m.invoice),
            invoiceNumber: m.invoice.invoice_number,
            dateTime: it.date_time,
            pickup: it.pickup,
            dropoff: it.dropoff,
            amount: it.amount,
          })
        }
      }
      suggestedRides = minTransferPick(rides, r => r.amount, ctOverRaw)
    }
    cityTransport = {
      usageRate: ctStandard > EPS ? (ctActual / ctStandard) * 100 : null,
      over: ctOver,
      overAmount: Math.max(ctOverRaw, 0),
      standard: ctStandard,
      actual: ctActual,
      dailyStd,
      days,
      suggestedInvoices,
      suggestedInvoicesTotal: sumBy(suggestedInvoices, s => s.amount),
      suggestedRides,
      suggestedRidesTotal: sumBy(suggestedRides, r => r.amount),
    }
  }

  // ── 住宿（有支出才进入分析） ────────────────────────────────
  let hotel: HotelOverage | null = null
  if (hotelActual > EPS) {
    const estNights = days > 1 ? days - 1 : 1
    const items: HotelOverageItem[] = []
    if (hotelDailyRate > EPS) {
      for (const m of trip.matches) {
        if (m.invoice.category !== 'Hotel') continue
        const detail = m.invoice.hotel_detail
        const hasDetail = (detail?.nights ?? 0) > 0
        const nights = hasDetail ? detail!.nights : estNights
        const estimated = !hasDetail
        const standard = hotelDailyRate * nights
        const overAmount = m.invoice.amount - standard
        if (overAmount > EPS) {
          items.push({
            invoiceId: m.invoice_id,
            label: invoiceLabel(m.invoice),
            dateRange:
              detail?.check_in && detail?.check_out
                ? `${detail.check_in.slice(5)} ~ ${detail.check_out.slice(5)}`
                : invoiceDate(m.invoice),
            nights,
            estimated,
            actual: m.invoice.amount,
            standard,
            overAmount,
            perNightOver: overAmount / nights,
          })
        }
      }
      items.sort((a, b) => b.overAmount - a.overAmount)
    }
    hotel = {
      usageRate: hotelStandard > EPS ? (hotelActual / hotelStandard) * 100 : null,
      over: items.length > 0,
      // 与明细一致：超标发票的超额合计（未超标发票不抵扣）
      overAmount: sumBy(items, i => i.overAmount),
      dailyRate: hotelDailyRate,
      standard: hotelStandard,
      actual: hotelActual,
      overNights: sumBy(items, i => i.nights),
      items,
    }
  }

  const over = cityTransport?.over === true || hotel?.over === true
  const overTotal = (cityTransport?.over ? cityTransport.overAmount : 0)
    + (hotel?.over ? hotel.overAmount : 0)

  return { over, overTotal, cityTransport, hotel }
}
