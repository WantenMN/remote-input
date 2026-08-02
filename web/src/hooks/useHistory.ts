import { useCallback, useEffect, useMemo, useState } from 'react'

const STORAGE_KEY = 'remote-input-history'
const RECORDING_KEY = 'remote-input-history-recording'

export interface HistoryItem {
  id: string
  text: string
  ts: number
  pinned: boolean
}

function generateId(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 6)
}

export function useHistory() {
  const [items, setItems] = useState<HistoryItem[]>(() => loadFromStorage())
  const [recording, setRecordingState] = useState<boolean>(() => {
    const val = localStorage.getItem(RECORDING_KEY)
    return val === null ? true : val === 'true'
  })

  const setRecording = useCallback((enabled: boolean) => {
    setRecordingState(enabled)
    localStorage.setItem(RECORDING_KEY, enabled ? 'true' : 'false')
  }, [])

  const addToHistory = useCallback((text: string) => {
    setItems((prev) => {
      const existing = prev.findIndex((item) => item.text === text)
      if (existing >= 0) {
        const next = [...prev]
        next[existing] = { ...next[existing], ts: Date.now() }
        return next
      }
      return [{ id: generateId(), text, ts: Date.now(), pinned: false }, ...prev]
    })
  }, [])

  const togglePin = useCallback((id: string) => {
    setItems((prev) =>
      prev.map((item) =>
        item.id === id ? { ...item, pinned: !item.pinned } : item,
      ),
    )
  }, [])

  const deleteItem = useCallback((id: string) => {
    setItems((prev) => prev.filter((item) => item.id !== id))
  }, [])

  const clearUnpinned = useCallback(() => {
    setItems((prev) => prev.filter((item) => item.pinned))
  }, [])

  const sortedItems = useMemo(
    () =>
      [...items].sort((a, b) => {
        if (a.pinned && !b.pinned) return -1
        if (!a.pinned && b.pinned) return 1
        return b.ts - a.ts
      }),
    [items],
  )

  // Persist on every change; kept out of the state updaters so updates stay pure.
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items))
  }, [items])

  return {
    items: sortedItems,
    recording,
    setRecording,
    addToHistory,
    togglePin,
    deleteItem,
    clearUnpinned,
  }
}

function loadFromStorage(): HistoryItem[] {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || '[]')
  } catch {
    return []
  }
}
