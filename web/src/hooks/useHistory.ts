import { useCallback, useState } from 'react'

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
      let next: HistoryItem[]
      if (existing >= 0) {
        next = [...prev]
        next[existing] = { ...next[existing], ts: Date.now() }
      } else {
        next = [{ id: generateId(), text, ts: Date.now(), pinned: false }, ...prev]
      }
      saveToStorage(next)
      return next
    })
  }, [])

  const togglePin = useCallback((id: string) => {
    setItems((prev) => {
      const next = prev.map((item) =>
        item.id === id ? { ...item, pinned: !item.pinned } : item,
      )
      saveToStorage(next)
      return next
    })
  }, [])

  const deleteItem = useCallback((id: string) => {
    setItems((prev) => {
      const next = prev.filter((item) => item.id !== id)
      saveToStorage(next)
      return next
    })
  }, [])

  const clearUnpinned = useCallback(() => {
    setItems((prev) => {
      const next = prev.filter((item) => item.pinned)
      saveToStorage(next)
      return next
    })
  }, [])

  const sortedItems = [...items].sort((a, b) => {
    if (a.pinned && !b.pinned) return -1
    if (!a.pinned && b.pinned) return 1
    return b.ts - a.ts
  })

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

function saveToStorage(items: HistoryItem[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(items))
}
