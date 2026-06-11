import { createFileRoute } from '@tanstack/react-router'
import { useCallback, useRef, useState } from 'react'
import { Trash2, History, Send, Zap } from 'lucide-react'
import { useWebSocket } from '@/hooks/useWebSocket'
import { useHistory } from '@/hooks/useHistory'
import { HistoryOverlay } from '@/components/HistoryOverlay'
import { Textarea } from '@/components/ui/textarea'
import { cn } from '@/lib/utils'

export const Route = createFileRoute('/')({
  component: IndexComponent,
})

const STATUS_LABELS = {
  connected: 'Connected',
  connecting: 'Connecting…',
  disconnected: 'Disconnected',
} as const

function IndexComponent() {
  const { status, send, onAck } = useWebSocket()
  const history = useHistory()
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const [historyOpen, setHistoryOpen] = useState(false)
  const [statusText, setStatusText] = useState('')
  const [sendSuccess, setSendSuccess] = useState(false)
  const statusTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const flashStatus = useCallback((msg: string, duration = 2500) => {
    if (statusTimer.current) clearTimeout(statusTimer.current)
    setStatusText(msg)
    statusTimer.current = setTimeout(() => setStatusText(''), duration)
  }, [])

  const focusInput = useCallback(() => {
    const el = textareaRef.current
    if (el) {
      el.focus()
      el.setSelectionRange(el.value.length, el.value.length)
    }
  }, [])

  const handleSend = useCallback(() => {
    const el = textareaRef.current
    if (!el) return
    const text = el.value.trim()
    if (!text) {
      focusInput()
      return
    }
    if (!send(text)) {
      flashStatus('Not connected')
      focusInput()
      return
    }
    if (history.recording) {
      history.addToHistory(text)
    }
    el.value = ''
    setSendSuccess(true)
    setTimeout(() => setSendSuccess(false), 400)
    focusInput()
  }, [send, history, focusInput, flashStatus])

  const handleClear = useCallback(() => {
    if (textareaRef.current) textareaRef.current.value = ''
    focusInput()
  }, [focusInput])

  const handleHistorySelect = useCallback(
    (text: string) => {
      const el = textareaRef.current
      if (el) {
        const existing = el.value
        if (existing && !existing.endsWith('\n')) {
          el.value = existing + '\n' + text
        } else {
          el.value = existing + text
        }
      }
      setHistoryOpen(false)
      focusInput()
    },
    [focusInput],
  )

  const handleHistoryDelete = useCallback(
    (id: string, text: string) => {
      const preview = text.length > 30 ? text.slice(0, 30) + '…' : text
      if (!confirm(`Delete "${preview}"?`)) return
      history.deleteItem(id)
    },
    [history],
  )

  const handleClearUnpinned = useCallback(() => {
    const total = history.items.length
    const pinned = history.items.filter((i) => i.pinned).length
    if (pinned === total) return
    if (!confirm(`Delete all unpinned history? (${total - pinned} items)`)) return
    history.clearUnpinned()
  }, [history])

  onAck((len) => flashStatus(`Pasted ${len} chars`))

  return (
    <div data-status={status} className="contents">
      {/* Animated gradient background */}
      <div className="absolute inset-0 bg-animated-gradient pointer-events-none" />

      {/* Header */}
      <header className="relative z-10 flex items-center justify-between shrink-0 px-5 pt-5 pb-3 select-none">
        <div className="flex items-center gap-2.5">
          <div className="flex items-center justify-center size-8 rounded-lg bg-primary/10">
            <Zap className="size-4 text-primary" />
          </div>
          <h1 className="text-[1.05rem] font-semibold tracking-tight">Remote Input</h1>
        </div>
        <div className="flex items-center gap-2 px-3 py-1.5 rounded-full glass-subtle">
          <span className={cn('status-dot', `status-dot-${status}`)} />
          <span className="text-xs font-medium text-primary">
            {STATUS_LABELS[status]}
          </span>
        </div>
      </header>

      {/* Textarea */}
      <div className="relative z-10 flex-1 min-h-0 px-4 py-2">
        <div className="h-full rounded-xl glass-subtle textarea-glow transition-all duration-300 overflow-hidden">
          <Textarea
            ref={textareaRef}
            placeholder="Type something… or use voice input on your phone"
            autoFocus
            className="h-full resize-none text-[15px] leading-relaxed bg-transparent border-0 focus-visible:ring-0 focus-visible:border-0 p-4 placeholder:text-muted-foreground/50"
          />
        </div>
      </div>

      {/* Toast status */}
      <div className="relative z-10 shrink-0 h-7 px-4 flex items-center justify-center">
        {statusText && (
          <div
            key={statusText}
            className="toast-enter flex items-center gap-1.5 px-3 py-1 rounded-full glass-subtle"
          >
            <span className="text-xs font-medium text-primary">{statusText}</span>
          </div>
        )}
      </div>

      {/* Bottom bar */}
      <div className="relative z-10 shrink-0 px-4 pb-safe pt-2">
        <div className="flex items-center gap-2.5 p-2 rounded-2xl glass">
          {/* Clear */}
          <button
            onClick={handleClear}
            className="icon-btn-ghost flex items-center justify-center size-12 rounded-xl text-muted-foreground hover:text-foreground active:text-foreground"
          >
            <Trash2 className="size-[18px]" />
          </button>

          {/* History */}
          <button
            onClick={() => setHistoryOpen(true)}
            className="icon-btn-ghost flex items-center justify-center size-12 rounded-xl text-muted-foreground hover:text-foreground active:text-foreground"
          >
            <History className="size-[18px]" />
          </button>

          {/* Send */}
          <button
            onClick={handleSend}
            className={cn(
              'send-btn flex-[2.5] flex items-center justify-center gap-2 h-12 rounded-xl',
              'bg-primary text-primary-foreground font-medium text-sm',
              'hover:brightness-110 active:brightness-95',
              sendSuccess && 'send-success-anim',
            )}
          >
            <span className="send-btn-glow" />
            <Send className="size-[18px] relative z-10" />
            <span className="relative z-10">Send</span>
          </button>
        </div>
      </div>

      <HistoryOverlay
        open={historyOpen}
        items={history.items}
        recording={history.recording}
        onRecordingChange={history.setRecording}
        onClose={() => {
          setHistoryOpen(false)
          focusInput()
        }}
        onSelect={handleHistorySelect}
        onTogglePin={history.togglePin}
        onDelete={handleHistoryDelete}
        onClearUnpinned={handleClearUnpinned}
      />
    </div>
  )
}
