import { useEffect, useRef, useCallback } from 'react'

export type StreamEvent =
  | { type: 'content'; payload: { text: string; chunk: boolean } }
  | { type: 'trace'; payload: { kind: string; [key: string]: unknown } }
  | { type: 'status'; payload: { kind: string; [key: string]: unknown } }

interface UseWebSocketOptions {
  onEvent: (event: StreamEvent) => void
  onClose?: () => void
}

export function useWebSocket(sessionId: string | null, opts: UseWebSocketOptions) {
  const wsRef = useRef<WebSocket | null>(null)
  const optsRef = useRef(opts)
  optsRef.current = opts

  useEffect(() => {
    if (!sessionId) return

    const ws = new WebSocket(`ws://localhost:8080/ws/sessions/${sessionId}`)
    wsRef.current = ws

    ws.onmessage = (e) => {
      try {
        const event = JSON.parse(e.data) as StreamEvent
        optsRef.current.onEvent(event)
      } catch { /* ignore parse errors */ }
    }

    ws.onclose = () => optsRef.current.onClose?.()

    return () => ws.close()
  }, [sessionId])

  const send = useCallback((msg: unknown) => {
    wsRef.current?.send(JSON.stringify(msg))
  }, [])

  return { send }
}
