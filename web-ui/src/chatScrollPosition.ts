export interface ChatScrollPosition {
  scrollTop: number
  atBottom: boolean
}

const STORAGE_PREFIX = 'llm-tutor.chat-scroll.'

export function loadChatScrollPosition(sessionId: string): ChatScrollPosition | null {
  try {
    return parseChatScrollPosition(window.localStorage.getItem(storageKey(sessionId)))
  } catch {
    return null
  }
}

export function saveChatScrollPosition(sessionId: string, position: ChatScrollPosition) {
  try {
    window.localStorage.setItem(storageKey(sessionId), JSON.stringify(position))
  } catch {
    // Scroll persistence is a convenience and must not interrupt chat.
  }
}

export function parseChatScrollPosition(value: string | null): ChatScrollPosition | null {
  if (!value) return null
  try {
    const parsed = JSON.parse(value) as Record<string, unknown>
    if (typeof parsed.scrollTop !== 'number' || !Number.isFinite(parsed.scrollTop)) return null
    if (typeof parsed.atBottom !== 'boolean') return null
    return {
      scrollTop: Math.max(0, parsed.scrollTop),
      atBottom: parsed.atBottom,
    }
  } catch {
    return null
  }
}

export function restoredScrollTop(
  position: ChatScrollPosition | null,
  scrollHeight: number,
  clientHeight: number,
) {
  const maximum = Math.max(0, scrollHeight - clientHeight)
  if (!position || position.atBottom) return maximum
  return Math.max(0, Math.min(position.scrollTop, maximum))
}

function storageKey(sessionId: string) {
  return `${STORAGE_PREFIX}${sessionId}`
}
