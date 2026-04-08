import { onMounted, onBeforeUnmount, ref } from 'vue'

/**
 * Monitors user activity and calls onIdle after timeout.
 * Tracks mousemove, keydown, mousedown, touchstart events.
 */
export function useIdleTimer(
  timeoutMinutes: number,
  onIdle: () => void,
) {
  const isIdle = ref(false)
  let timer: ReturnType<typeof setTimeout> | null = null

  function resetTimer() {
    if (isIdle.value) return // Don't reset if already locked
    if (timer) clearTimeout(timer)
    if (timeoutMinutes <= 0) return // 0 = disabled
    timer = setTimeout(() => {
      isIdle.value = true
      onIdle()
    }, timeoutMinutes * 60 * 1000)
  }

  function resume() {
    isIdle.value = false
    resetTimer()
  }

  const events = ['mousemove', 'keydown', 'mousedown', 'touchstart', 'scroll']

  onMounted(() => {
    events.forEach(e => window.addEventListener(e, resetTimer, { passive: true }))
    resetTimer()
  })

  onBeforeUnmount(() => {
    if (timer) clearTimeout(timer)
    events.forEach(e => window.removeEventListener(e, resetTimer))
  })

  return { isIdle, resume }
}
