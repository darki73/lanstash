/**
 * Bars: a ranked list where each row's share is drawn as a bar under it: a group heading with its
 * total, then its rows: a small picture when given (a game's icon), a name with a muted detail, the
 * figure, and a bar as long as the row's fraction of the largest (0..1, clamped; a row above 0
 * always shows a sliver of 0.6 %), in one tone or split in two (what came from the cache, the rest).
 * A row's name may link to its own page. A group may end in a line under its rows (a link to all).
 */
import type { ReactNode } from "react"
import { fraction as clamp } from "@/lib/clamp"
import { classes } from "@/lib/classes"
import { type Tone, toneBg } from "@/styled/tone"

export interface BarGroupProps {
  readonly title: string
  readonly value: string
  readonly detail?: string
  /** A line under the rows, such as a link to all of them. */
  readonly more?: ReactNode
  readonly children?: ReactNode
}

export function BarGroup({ title, value, detail, more, children }: BarGroupProps) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between gap-4 border-b pb-1.5">
        <span className="font-mono text-[11px] text-faint uppercase tracking-[0.14em]">{title}</span>
        <span className="font-mono text-xs tabular-nums">
          {value}
          {detail !== undefined && <span className="ms-2 text-faint">{detail}</span>}
        </span>
      </div>
      <ul className="flex flex-col gap-2.5">{children}</ul>
      {more !== undefined && <div className="pt-1">{more}</div>}
    </div>
  )
}

export interface BarRowProps {
  readonly name: string
  readonly detail?: string
  readonly value: string
  readonly sub?: string
  readonly fraction: number
  readonly tone: Tone
  /** A small picture before the name (a game's icon). */
  readonly icon?: ReactNode
  /** The bar split in two: `share` (0..1) of it in `tone`, the rest in `rest`. */
  readonly split?: { readonly share: number; readonly rest: Tone }
  /** The name is a link to this page (the game's). */
  readonly href?: string
}

export function BarRow({ name, detail, value, sub, fraction, tone, icon, split, href }: BarRowProps) {
  const pct = 100 * clamp(fraction)
  const share = split === undefined ? 1 : clamp(split.share)
  return (
    <li className="flex items-center gap-3">
      {icon !== undefined && <span className="inline-flex w-8 shrink-0 justify-center">{icon}</span>}
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="flex items-baseline justify-between gap-4">
          <span className="flex min-w-0 items-baseline gap-2">
            {href === undefined ? (
              <span className="truncate">{name}</span>
            ) : (
              <a
                href={href}
                className="truncate rounded-sm underline-offset-2 hover:text-cache hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cache"
              >
                {name}
              </a>
            )}
            {detail !== undefined && <span className="truncate font-mono text-faint text-xs">{detail}</span>}
          </span>
          <span className="shrink-0 font-mono text-xs tabular-nums">
            {value}
            {sub !== undefined && <span className="ms-2 text-faint">{sub}</span>}
          </span>
        </div>
        <div className="h-1 overflow-hidden rounded-full bg-sunken">
          <div data-bar className="flex h-full" style={{ width: `${pct > 0 ? Math.max(pct, 0.6) : 0}%` }}>
            {share > 0 && (
              <div
                className={classes("h-full first:rounded-s-full last:rounded-e-full", toneBg[tone])}
                style={{ width: `${100 * share}%` }}
              />
            )}
            {split !== undefined && share < 1 && (
              <div
                className={classes("h-full first:rounded-s-full last:rounded-e-full", toneBg[split.rest])}
                style={{ width: `${100 * (1 - share)}%` }}
              />
            )}
          </div>
        </div>
      </div>
    </li>
  )
}
