import { ref, watch, onUnmounted } from 'vue'

/**
 * 统计数字滚动动画：目标值变化时从旧值平滑滚到新值。
 * 返回 ref，模板中直接使用。
 */
export function useCountUp(target: () => number, duration = 700) {
  const value = ref(target())
  let raf = 0
  let from = value.value

  function animate(to: number) {
    cancelAnimationFrame(raf)
    const start = performance.now()
    const delta = to - from
    if (delta === 0) return
    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / duration)
      const eased = 1 - Math.pow(1 - t, 3) // ease-out cubic
      value.value = from + delta * eased
      if (t < 1) raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
  }

  watch(target, (v) => {
    from = value.value
    animate(v)
  })

  onUnmounted(() => cancelAnimationFrame(raf))

  return value
}
