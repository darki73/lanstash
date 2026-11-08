/**
 * Tile: a resource in the system-monitor style: its name, its figure, a line of detail and a small
 * chart, as a link to its page. The tile of the page shown is raised and marked in its tone.
 * TileRow lays tiles out side by side: two to a row on a phone, all five on a wide screen.
 */
import type { ReactNode } from "react"
import { classes } from "@/lib/classes"
import { type Tone, toneBg } from "@/styled/tone"

export interface TileProps {
  readonly name: string
  readonly value: string
  readonly detail: string
  readonly tone: Tone
  readonly href: string
  readonly current: boolean
  /** The small chart. */
  readonly children?: ReactNode
}

export function Tile({ name, value, detail, tone, href, current, children }: TileProps) {
  return (
    <a
      href={href}
      aria-current={current ? "page" : undefined}
      className={classes(
        "group relative flex min-w-0 flex-col gap-2 overflow-hidden rounded-xl border px-3.5 pt-3 pb-0 transition-colors",
        current ? "border-line bg-raised shadow-panel" : "bg-raised/40 hover:bg-raised",
      )}
    >
      <span
        aria-hidden
        className={classes(
          "absolute inset-x-3.5 top-0 h-0.5 rounded-b-full transition-opacity",
          toneBg[tone],
          current ? "opacity-100" : "opacity-0 group-hover:opacity-40",
        )}
      />
      <span className="flex min-w-0 flex-col">
        <span className="font-medium text-muted text-xs">{name}</span>
        <span className="truncate font-mono text-base tabular-nums">{value}</span>
        <span className="truncate text-faint text-xs">{detail}</span>
      </span>
      <span className="-mx-3.5 h-9 overflow-hidden">{children}</span>
    </a>
  )
}

export function TileRow({ label, children }: { readonly label: string; readonly children?: ReactNode }) {
  return (
    <nav aria-label={label} className="grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-5">
      {children}
    </nav>
  )
}
