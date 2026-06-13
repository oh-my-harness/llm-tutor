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
  const pendingRef = useRef<string[]>([])
  const optsRef = useRef(opts)
  optsRef.current = opts

  useEffect(() => {
    if (!sessionId) return

    const ws = new WebSocket(`ws://localhost:8080/ws/sessions/${sessionId}`)
    wsRef.current = ws

    ws.onopen = () => {
      for (const item of pendingRef.current) {
        ws.send(item)
      }
      pendingRef.current = []
    }

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
    const payload = JSON.stringify(msg)
    const ws = wsRef.current
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(payload)
    } else {
      pendingRef.current.push(payload)
    }
  }, [])

  return { send }
}
