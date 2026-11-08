/**
 * View: the frame of one page's content: its name large (with its mark before it, if it has one)
 * and a line under it, what it is about on the right, then its parts one under the other, rising
 * into place when the view opens.
 */
import type { ReactNode } from "react"

export interface ViewProps {
  readonly title: string
  readonly subtitle: string
  /** Words on the right of the title, such as the figure in the title's unit. */
  readonly aside?: ReactNode
  /** A mark before the title. */
  readonly icon?: ReactNode
  readonly children?: ReactNode
}

export function View({ title, subtitle, aside, icon, children }: ViewProps) {
  return (
    <section aria-label={title} className="flex animate-rise flex-col gap-5">
      <header className="flex flex-wrap items-end justify-between gap-x-6 gap-y-1 border-b pb-3">
        <div className="flex items-center gap-3.5">
          {icon}
          <div className="flex flex-col">
            <h2 className="font-semibold text-2xl tracking-tight">{title}</h2>
            <p className="text-muted">{subtitle}</p>
          </div>
        </div>
        {aside !== undefined && <div className="font-mono text-muted text-sm tabular-nums">{aside}</div>}
      </header>
      {children}
    </section>
  )
}
