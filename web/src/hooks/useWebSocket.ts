import { useCallback, useEffect, useRef, useState } from 'react'

export type WsStatus = 'connected' | 'disconnected' | 'connecting'

export function useWebSocket() {
  const [status, setStatus] = useState<WsStatus>('disconnected')
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const onAckRef = useRef<((len: number) => void) | null>(null)

  const connect = useCallback(() => {
    if (wsRef.current && wsRef.current.readyState <= 1) return

    if (reconnectTimer.current) {
      clearTimeout(reconnectTimer.current)
      reconnectTimer.current = null
    }

    setStatus('connecting')

    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const ws = new WebSocket(proto + '//' + location.host + '/ws')
    wsRef.current = ws

    ws.onopen = () => setStatus('connected')

    ws.onmessage = (evt) => {
      try {
        const data = JSON.parse(evt.data)
        if (data.ok && onAckRef.current) {
          onAckRef.current(data.len)
        }
      } catch { /* ignore */ }
    }

    ws.onclose = () => {
      setStatus('disconnected')
      scheduleReconnect()
    }

    ws.onerror = () => ws.close()
  }, [])

  const scheduleReconnect = useCallback(() => {
    if (reconnectTimer.current) clearTimeout(reconnectTimer.current)
    reconnectTimer.current = setTimeout(connect, 2000)
  }, [connect])

  const send = useCallback((text: string) => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) {
      return false
    }
    wsRef.current.send(text)
    return true
  }, [])

  const onAck = useCallback((cb: (len: number) => void) => {
    onAckRef.current = cb
  }, [])

  useEffect(() => {
    connect()
    return () => {
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current)
      if (wsRef.current) wsRef.current.close()
    }
  }, [connect])

  return { status, send, onAck }
}
