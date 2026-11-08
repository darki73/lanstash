/**
 * Machine: a card for a PC on the network. Across its top, a soft wash in a hue of the PC's own
 * (worked out from its name, so the same PC keeps its colour); over it, a tile in that hue with the
 * name's initials, the name large, a line under it in the monospace face (its address), and an
 * action on the right (Rename). Under that, an optional part that replaces the header's line (a
 * form to rename it), then the card's content, with a summary in a column on its left when given
 * (over it on a narrow screen).
 */
import type { ReactNode } from "react"
import { hueOf, initialsOf } from "@/styled/cards"

export interface MachineProps {
  readonly name: string
  /** The line under the name. */
  readonly detail: string
  readonly action?: ReactNode
  /** Shown under the header (a form), when given. */
  readonly editing?: ReactNode
  /** A summary on the left of the content where the card is wide (over it where narrow). */
  readonly side?: ReactNode
  readonly children?: ReactNode
}

export function Machine({ name, detail, action, editing, side, children }: MachineProps) {
  const hue = hueOf(name)
  return (
    <section aria-label={name} className="relative isolate overflow-hidden rounded-xl border bg-raised shadow-panel">
      <div
        aria-hidden
        className="-z-10 absolute inset-x-0 top-0 h-24"
        style={{
          backgroundImage: `linear-gradient(180deg, hsl(${hue} 55% 45% / 0.22), transparent), repeating-linear-gradient(135deg, hsl(${hue} 40% 50% / 0.05) 0 2px, transparent 2px 14px)`,
        }}
      />
      <div className="flex items-center gap-3 px-4 pt-4">
        <span
          aria-hidden
          className="inline-flex size-11 shrink-0 items-center justify-center rounded-xl border font-semibold text-sm text-text/90 shadow-panel"
          style={{
            backgroundImage: `linear-gradient(135deg, hsl(${hue} 55% 48% / 0.85), hsl(${(hue + 50) % 360} 50% 32% / 0.85))`,
          }}
        >
          {initialsOf(name) || "?"}
        </span>
        <div className="flex min-w-0 flex-1 flex-col">
          <h3 className="truncate font-semibold text-lg tracking-tight">{name}</h3>
          <span className="truncate font-mono text-muted text-xs">{detail}</span>
        </div>
        {action !== undefined && <div className="shrink-0">{action}</div>}
      </div>
      {editing !== undefined && <div className="px-4 pt-3">{editing}</div>}
      {side === undefined ? (
        <div className="flex flex-col gap-4 p-4">{children}</div>
      ) : (
        <div className="grid gap-6 p-4 lg:grid-cols-[minmax(0,19rem)_minmax(0,1fr)] lg:gap-8">
          <div className="min-w-0 lg:border-r lg:pr-8">{side}</div>
          <div className="flex min-w-0 flex-col gap-4">{children}</div>
        </div>
      )}
    </section>
  )
}
