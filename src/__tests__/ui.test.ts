import { describe, it, expect, vi } from 'vitest'

describe('toast', () => {
  it('push 后 3 秒自动移除', async () => {
    vi.useFakeTimers()
    const { toast, toasts } = await import('../composables/toast')
    toast('成功', 'success')
    expect(toasts.value).toHaveLength(1)
    expect(toasts.value[0].message).toBe('成功')
    vi.advanceTimersByTime(3000)
    expect(toasts.value).toHaveLength(0)
    vi.useRealTimers()
  })
})
