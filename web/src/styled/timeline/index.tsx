/**
 * Timeline: what happened, newest first, grouped by day. Each day has a heading line; under it
 * runs a thin rail with a dot per entry, the dot in the entry's kind's colour (the kind is
 * always written too, in a pill). An entry is a time in the monospace face, an icon, a title (a
 * link where there is a page) with a line under it, and on the right the pill and what it is about
 * (a version, an old one struck through before it). On a narrow screen the pill and the detail
 * wrap under the title.
 */
import type { ReactNode } from "react"
import { classes } from "@/lib/classes"

/** What kind of thing happened: new (a new build), done (whole now), seen (first seen). */
export type TimelineKind = "new" | "done" | "seen"

export interface TimelineEntry {
  readonly key: string
  /** The time of day ("14:32"). */
  readonly time: string
  readonly icon?: ReactNode
  readonly title: string
  /** The title's link, where there is a page. */
  readonly href?: string
  readonly subtitle?: string
  readonly kind: TimelineKind
  /** The kind in words ("new build"). */
  readonly label: string
  /** What it is about now ("v7.3.63.1"). */
  readonly detail: string
  /** What it was before, struck through before the detail. */
  readonly before?: string
}

export interface TimelineDay {
  /** "Today", "Yesterday", "Mon 3 Nov". */
  readonly label: string
  readonly entries: readonly TimelineEntry[]
}

const dot: Readonly<Record<TimelineKind, string>> = {
  new: "bg-origin",
  done: "bg-success",
  seen: "bg-cache",
}

const pill: Readonly<Record<TimelineKind, string>> = {
  new: "border-origin/40 text-origin",
  done: "border-success/40 text-success",
  seen: "border-cache/40 text-cache",
}

export function Timeline({ days, label }: { readonly days: readonly TimelineDay[]; readonly label: string }) {
  return (
    <section aria-label={label} className="flex animate-rise flex-col gap-6">
      {days.map((day) => (
        <div key={day.label} className="flex flex-col gap-2">
          <h3 className="flex items-center gap-3 font-mono text-[11px] text-faint uppercase tracking-[0.14em]">
            <span className="shrink-0">{day.label}</span>
            <span aria-hidden className="h-px flex-1 bg-line" />
          </h3>
          <ol className="relative flex flex-col">
            <span aria-hidden className="absolute top-3 bottom-3 left-[calc(4.375rem-0.5px)] w-px bg-line" />
            {day.entries.map((e) => (
              <li
                key={e.key}
                className="relative grid grid-cols-[3.25rem_1.25rem_minmax(0,1fr)] items-start gap-x-2 rounded-lg py-2 pr-2 hover:bg-hover"
              >
                <span className="pt-0.5 text-right font-mono text-muted text-xs tabular-nums">{e.time}</span>
                <span className="flex justify-center pt-1.5">
                  <span aria-hidden className={classes("z-10 size-2 rounded-full ring-4 ring-raised", dot[e.kind])} />
                </span>
                <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1.5">
                  <div className="flex min-w-0 flex-1 basis-56 items-center gap-2.5">
                    {e.icon}
                    <div className="flex min-w-0 flex-col">
                      {e.href === undefined ? (
                        <span className="truncate font-medium text-sm">{e.title}</span>
                      ) : (
                        <a href={e.href} className="truncate font-medium text-sm hover:underline">
                          {e.title}
                        </a>
                      )}
                      {e.subtitle !== undefined && <span className="truncate text-muted text-xs">{e.subtitle}</span>}
                    </div>
                  </div>
                  <div className="flex min-w-0 items-center gap-2.5">
                    <span
                      className={classes(
                        "whitespace-nowrap rounded-full border px-2 py-0.5 font-mono text-[11px]",
                        pill[e.kind],
                      )}
                    >
                      {e.label}
                    </span>
                    <span className="flex min-w-0 items-baseline gap-1.5 font-mono text-xs tabular-nums">
                      {e.before !== undefined && (
                        <>
                          <span className="truncate text-muted line-through">{e.before}</span>
                          <span aria-hidden className="text-muted">
                            →
                          </span>
                        </>
                      )}
                      <span className="truncate">{e.detail}</span>
                    </span>
                  </div>
                </div>
              </li>
            ))}
          </ol>
        </div>
      ))}
    </section>
  )
}
