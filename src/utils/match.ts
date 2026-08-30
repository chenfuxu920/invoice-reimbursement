import type { MatchResult } from '../types'

/// 统计一条匹配结果中未配上支付的行程数。
/// 仅依据 itinerary_payment_pairs 显式配对判定：配对表为空（旧数据或
/// 子集和整单支付场景）时无法判定，返回 0；配对表非空但缺某行程下标
/// 即该行程未配上支付。
export function countUnmatchedItineraries(match: MatchResult): number {
  const total = match.invoice.itineraries.length
  if (total === 0) return 0
  const pairs = match.itinerary_payment_pairs || []
  if (pairs.length === 0) return 0
  const paired = new Set(pairs.map(p => p.itinerary_index))
  let count = 0
  for (let i = 0; i < total; i++) {
    if (!paired.has(i)) count++
  }
  return count
}

/// 是否存在影响导出完整性的未处理项：未匹配发票，或已匹配发票中有未配上支付的行程。
export function hasExportGaps(
  unmatchedInvoiceCount: number,
  matches: MatchResult[],
): boolean {
  if (unmatchedInvoiceCount > 0) return true
  return matches.some(m => countUnmatchedItineraries(m) > 0)
}
