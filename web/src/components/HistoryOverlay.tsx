import { useEffect, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Pin, Trash2, ChevronDown, ChevronUp, ArrowLeft, Clock, CircleDot } from 'lucide-react'
import type { HistoryItem } from '@/hooks/useHistory'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/utils'

interface Props {
  open: boolean
  items: HistoryItem[]
  recording: boolean
  onRecordingChange: (enabled: boolean) => void
  onClose: () => void
  onSelect: (text: string) => void
  onTogglePin: (id: string) => void
  onDelete: (id: string, text: string) => void
  onClearUnpinned: () => void
}

function formatTime(ts: number): string {
  const d = new Date(ts)
  const pad = (n: number) => (n < 10 ? '0' + n : String(n))
  const now = new Date()
  const isToday = d.toDateString() === now.toDateString()
  if (isToday) return `Today ${pad(d.getHours())}:${pad(d.getMinutes())}`
  const yesterday = new Date(now)
  yesterday.setDate(yesterday.getDate() - 1)
  if (d.toDateString() === yesterday.toDateString()) return `Yesterday ${pad(d.getHours())}:${pad(d.getMinutes())}`
  return `${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

const CLAMP_LEN = 200

function HistoryItemCard({
  item,
  onSelect,
  onTogglePin,
  onDelete,
}: {
  item: HistoryItem
  onSelect: () => void
  onTogglePin: () => void
  onDelete: () => void
}) {
  const [expanded, setExpanded] = useState(false)
  const needsClamp = item.text.length > CLAMP_LEN

  return (
    <div
      className={cn(
        'group relative flex items-start gap-3 rounded-xl p-3.5 shrink-0 transition-colors duration-200',
        'glass-subtle hover:bg-white/[0.04]',
        item.pinned && 'pinned-glow',
      )}
    >
      <div className="flex-1 min-w-0 cursor-pointer" onClick={onSelect}>
        <p
          className={cn(
            'text-[13px] leading-relaxed text-foreground/90 whitespace-pre-wrap break-all',
            !expanded && needsClamp && 'line-clamp-7',
          )}
        >
          {item.text}
        </p>
        <div className="flex items-center gap-3 text-[11px] text-muted-foreground mt-2">
          <span className="flex items-center gap-1">
            {item.pinned ? (
              <Pin className="size-3 text-warning" fill="currentColor" />
            ) : (
              <Clock className="size-3" />
            )}
            {formatTime(item.ts)}
          </span>
          <span className="opacity-50">·</span>
          <span>{item.text.length} chars</span>
        </div>
      </div>
      <div className={cn(
        'flex shrink-0 gap-1 opacity-60 group-hover:opacity-100 transition-opacity duration-200',
        needsClamp ? 'flex-col' : 'flex-row',
      )}>
        {needsClamp && (
          <button
            className="icon-btn-ghost flex items-center justify-center size-7 rounded-lg text-muted-foreground hover:text-foreground"
            onClick={() => setExpanded(!expanded)}
          >
            {expanded ? <ChevronUp className="size-3.5" /> : <ChevronDown className="size-3.5" />}
          </button>
        )}
        <button
          className={cn(
            'icon-btn-ghost flex items-center justify-center size-7 rounded-lg',
            item.pinned ? 'text-warning' : 'text-muted-foreground hover:text-foreground',
          )}
          onClick={onTogglePin}
        >
          <Pin className="size-3.5" fill={item.pinned ? 'currentColor' : 'none'} />
        </button>
        <button
          className="icon-btn-ghost flex items-center justify-center size-7 rounded-lg text-muted-foreground hover:text-destructive"
          onClick={onDelete}
        >
          <Trash2 className="size-3.5" />
        </button>
      </div>
    </div>
  )
}

export function HistoryOverlay({
  open,
  items,
  recording,
  onRecordingChange,
  onClose,
  onSelect,
  onTogglePin,
  onDelete,
  onClearUnpinned,
}: Props) {
  const scrollRef = useRef<HTMLDivElement>(null)

  const rowVirtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 140,
    overscan: 6,
  })

  useEffect(() => {
    if (open && scrollRef.current) scrollRef.current.scrollTop = 0
  }, [open])

  return (
    <div
      className={cn(
        'fixed inset-0 z-50 flex flex-col transition-all duration-300 ease-out',
        open
          ? 'opacity-100 translate-y-0 pointer-events-auto'
          : 'opacity-0 translate-y-full pointer-events-none',
      )}
    >
      <div className="absolute inset-0 bg-background" />

      <header className="relative z-10 flex items-center justify-between shrink-0 px-5 pt-5 pb-3 select-none">
        <div className="flex items-center gap-3">
          <button
            onClick={onClose}
            className="icon-btn-ghost flex items-center justify-center size-9 -ml-1 rounded-xl text-muted-foreground hover:text-foreground"
          >
            <ArrowLeft className="size-5" />
          </button>
          <h1 className="text-base font-semibold flex items-center gap-2">
            <Clock className="size-4 text-muted-foreground" />
            History
            {items.length > 0 && (
              <span className="text-xs font-normal text-muted-foreground bg-white/[0.05] px-2 py-0.5 rounded-full">
                {items.length}
              </span>
            )}
          </h1>
        </div>
        <div className="flex items-center gap-2.5">
          <span className={cn(
            'text-xs font-medium',
            recording ? 'text-primary' : 'text-muted-foreground',
          )}>
            {recording ? 'Recording' : 'Paused'}
          </span>
          <div className="flex items-center gap-1.5">
            <CircleDot className={cn('size-3 transition-colors', recording ? 'text-primary' : 'text-muted-foreground')} />
            <Switch checked={recording} onCheckedChange={onRecordingChange} />
          </div>
        </div>
      </header>

      <div className="relative z-10 h-px bg-white/[0.04] mx-5" />

      <div ref={scrollRef} className="relative z-10 flex-1 overflow-y-auto min-h-0 custom-scrollbar">
        {items.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-48 gap-3">
            <div className="size-12 rounded-full bg-white/[0.03] flex items-center justify-center">
              <Clock className="size-5 text-muted-foreground/40" />
            </div>
            <p className="text-sm text-muted-foreground/60">No history yet</p>
            <p className="text-xs text-muted-foreground/40">Sent messages will appear here</p>
          </div>
        ) : (
          <div
            className="relative w-full"
            style={{ height: rowVirtualizer.getTotalSize() }}
          >
            {rowVirtualizer.getVirtualItems().map((row) => {
              const item = items[row.index]
              return (
                <div
                  key={item.id}
                  data-index={row.index}
                  ref={rowVirtualizer.measureElement}
                  className="absolute left-0 right-0 top-0 px-3"
                  style={{ transform: `translateY(${row.start}px)` }}
                >
                  <div className="pb-1.5">
                    <HistoryItemCard
                      item={item}
                      onSelect={() => onSelect(item.text)}
                      onTogglePin={() => onTogglePin(item.id)}
                      onDelete={() => onDelete(item.id, item.text)}
                    />
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>

      <div className="relative z-10 shrink-0 px-4 pb-safe pt-2">
        <div className="flex items-center gap-2.5 p-2 rounded-2xl glass">
          <button
            className="icon-btn-ghost flex items-center justify-center size-12 rounded-xl text-muted-foreground hover:text-destructive"
            onClick={onClearUnpinned}
          >
            <Trash2 className="size-[18px]" />
          </button>
          <button
            className="send-btn flex-[2.5] flex items-center justify-center gap-2 h-12 rounded-xl bg-secondary text-secondary-foreground font-medium text-sm hover:bg-white/[0.08] active:bg-white/[0.04]"
            onClick={onClose}
          >
            <ArrowLeft className="size-[18px]" />
            <span>Back</span>
          </button>
        </div>
      </div>
    </div>
  )
}
