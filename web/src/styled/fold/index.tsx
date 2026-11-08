/**
 * Fold: a part that starts closed, its title and a summary on one line, opened by clicking it
 * (a native details element: it opens with the keyboard too).
 */
import type { ReactNode } from "react"

export interface FoldProps {
  readonly title: string
  readonly summary?: string
  readonly children?: ReactNode
}

export function Fold({ title, summary, children }: FoldProps) {
  return (
    <details className="group rounded-xl border bg-raised shadow-panel">
      <summary className="flex cursor-pointer list-none items-center justify-between gap-4 px-4 py-3 [&::-webkit-details-marker]:hidden">
        <span className="flex items-center gap-2">
          <span aria-hidden className="text-faint transition-transform group-open:rotate-90">
            ▸
          </span>
          <span className="font-mono text-[11px] text-faint uppercase tracking-[0.14em]">{title}</span>
        </span>
        {summary !== undefined && <span className="font-mono text-muted text-xs tabular-nums">{summary}</span>}
      </summary>
      <div className="flex flex-col gap-3 border-t px-4 py-3">{children}</div>
    </details>
  )
}
