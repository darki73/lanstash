/**
 * Action bar: what is chosen and what can be done with it, in a bar that stays at the bottom of the
 * view while the list above it scrolls: the words on the left (a summary, a warning), the buttons on
 * the right; on a narrow screen they stack.
 */
import type { ReactNode } from "react"

export interface ActionBarProps {
  /** What the bar is about, for assistive technology. */
  readonly label: string
  /** The summary on the left. */
  readonly children?: ReactNode
  /** The buttons on the right. */
  readonly actions?: ReactNode
}

export function ActionBar({ label, children, actions }: ActionBarProps) {
  return (
    <section
      aria-label={label}
      className="sticky bottom-14 z-10 flex flex-col gap-3 rounded-lg border bg-raised/95 px-4 py-3 shadow-panel backdrop-blur-md sm:flex-row sm:items-center sm:justify-between"
    >
      <div className="flex min-w-0 flex-col gap-1 text-sm">{children}</div>
      <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div>
    </section>
  )
}
