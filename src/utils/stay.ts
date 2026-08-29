/**
 * 住宿天数与行程天数核对（纯函数，供导出页 TripCard 提示与摘要芯片使用）。
 *
 * 口径与后端 build_reimbursement_form 一致：
 * - 行程天数：travelStart ~ travelEnd 含首尾两天的自然日数（日期倒挂时按 1 天计）；
 * - 住宿天数：趟内住宿发票 hotel_detail.nights 之和；
 * - 对应关系：行程 N 天应有 N-1 晚住宿。
 *
 * 发票缺少入住/离店明细时无法真实核对（后端会按行程天数估算晚数），
 * 此时返回 incomplete 状态提示补充信息，避免漏报。
 */
import type { Trip } from '../types'

export interface StayDayCheck {
  /** mismatch：住宿晚数与行程对不上；incomplete：发票缺少入住/离店明细，无法核对 */
  status: 'mismatch' | 'incomplete'
  /** 行程天数（含首尾） */
  tripDays: number
  /** 行程天数对应的住宿晚数（行程天数 - 1） */
  expectedNights: number
  /** 已识别明细的住宿晚数合计 */
  nights: number
  /** 缺少入住/离店明细的住宿发票张数 */
  unknownCount: number
}

function parseDay(s: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(s.trim())
  if (!m) return null
  const t = Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3]))
  return Number.isFinite(t) ? t : null
}

/**
 * 核对一趟出差的住宿天数与行程天数。
 * 返回 null 表示无需提示：没有住宿发票、行程日期不完整，或两者正好对应。
 */
export function analyzeStayDays(trip: Trip): StayDayCheck | null {
  const start = parseDay(trip.travelStart)
  const end = parseDay(trip.travelEnd)
  if (start === null || end === null) return null

  const tripDays = Math.max(Math.round((end - start) / 86_400_000) + 1, 1)
  const expectedNights = tripDays - 1

  const hotels = trip.matches.filter(m => m.invoice.category === 'Hotel')
  if (hotels.length === 0) return null

  let nights = 0
  let unknownCount = 0
  for (const m of hotels) {
    const n = m.invoice.hotel_detail?.nights ?? 0
    if (n > 0) nights += n
    else unknownCount += 1
  }

  if (unknownCount > 0) {
    return { status: 'incomplete', tripDays, expectedNights, nights, unknownCount }
  }
  if (nights !== expectedNights) {
    return { status: 'mismatch', tripDays, expectedNights, nights, unknownCount }
  }
  return null
}
