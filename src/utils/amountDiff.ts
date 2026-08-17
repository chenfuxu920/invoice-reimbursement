/// 金额差颜色分级：
/// - 无差异（<= 0.01）：灰色
/// - 0.01 ~ 1 元：黄色
/// - 1 ~ 10 元：橙色
/// - 超过 10 元：红色
export function amountDiffClass(diff: number): string {
  const abs = Math.abs(diff)
  if (abs <= 0.01) return 'text-slate-400'
  if (abs <= 1) return 'text-amber-500'
  if (abs <= 10) return 'text-orange-500'
  return 'text-rose-500'
}

/// 头部 chip 使用的金额差颜色分级（带背景/边框）
export function amountDiffChipClass(diff: number): string {
  const abs = Math.abs(diff)
  if (abs <= 1) return 'bg-amber-50 text-amber-600 border-amber-200/70'
  if (abs <= 10) return 'bg-orange-50 text-orange-600 border-orange-200/70'
  return 'bg-rose-50 text-rose-600 border-rose-200/70'
}
