/**
 * Panel: a part of a view on a raised card, under a label in small spaced capitals, with a note
 * and an action on the right of the label (such as how old its figures are, a Stop button); PanelGrid lays panels out in as
 * many columns as fit. A panel's content scrolls sideways when it is wider than the panel.
 */
import type { ReactNode } from "react"

export interface PanelProps {
  readonly title: string
  readonly note?: string
  /** An action beside the title, such as a Button. */
  readonly action?: ReactNode
  readonly children?: ReactNode
}

export function Panel({ title, note, action, children }: PanelProps) {
  return (
    <section aria-label={title} className="overflow-x-auto rounded-xl border bg-raised p-4 shadow-panel">
      <div className="mb-3 flex items-baseline justify-between gap-4">
        <h3 className="font-mono text-[11px] text-faint uppercase tracking-[0.14em]">{title}</h3>
        <span className="flex items-center gap-3">
          {note !== undefined && <span className="text-faint text-xs">{note}</span>}
          {action}
        </span>
      </div>
      {children}
    </section>
  )
}

/** Columns of panels: as many as fit; a lone panel keeps one column's width. Wide: fewer, wider. */
export function PanelGrid({ wide, children }: { readonly wide?: boolean; readonly children?: ReactNode }) {
  return (
    <div
      className={
        wide
          ? "grid grid-cols-[repeat(auto-fit,minmax(22rem,1fr))] gap-x-8 gap-y-6"
          : "grid grid-cols-[repeat(auto-fill,minmax(19rem,1fr))] gap-3"
      }
    >
      {children}
    </div>
  )
}
