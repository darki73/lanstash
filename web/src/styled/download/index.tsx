/**
 * Download: a download in progress, drawn like a launcher's: a wide band with the game's art behind
 * it (darkened towards the right so the words stay readable; a plain band without art), the game's
 * name large on the left with what is being done over it, and on the right the live readings in a
 * row (label in small capitals over a figure, a swatch in its tone), a little bar graph of the last
 * moments (each bar split into its tones, stacked: what came from the stash or lancache in amber,
 * from the internet in cyan; scaled to the tallest), then a line with the state on the left and the
 * percentage on the right over a thin bar (in the origin colour unless another tone is given), and
 * the action (Stop) at the bottom right. Compact, it
 * leaves out the readings, the graph and the sharp art (only the blurred one stays behind the words):
 * a slimmer band for the top of another page.
 */
import { type ReactNode, useState } from "react"
import { classes } from "@/lib/classes"
import { type Tone, toneBg } from "@/styled/tone"

export interface Reading {
  readonly label: string
  readonly value: string
  readonly tone?: Tone
}

/** One moment of the graph: an amount per tone (any unit; bars are scaled to the tallest). */
export type Moment = Readonly<Partial<Record<Tone, number>>>

export interface DownloadProps {
  /** The game's name. */
  readonly title: string
  /** What is being done ("Import from lancache"). */
  readonly doing: string
  /** The game's art, if any. */
  readonly image?: string
  readonly readings?: readonly Reading[]
  /** The graph, oldest first. */
  readonly moments?: readonly Moment[]
  /** 0 to 1. */
  readonly fraction: number
  /** The line over the bar: the state ("4 minutes left"), and the percentage. */
  readonly state: string
  readonly percent: string
  readonly action?: ReactNode
  readonly compact?: boolean
  /** The bar's tone (origin: it fetches from the internet). */
  readonly barTone?: Tone
}

const stack: readonly Tone[] = ["cache", "origin", "plain"]

function Graph({ moments }: { readonly moments: readonly Moment[] }) {
  const total = (m: Moment) => stack.reduce((a, t) => a + (m[t] ?? 0), 0)
  const top = Math.max(1, ...moments.map(total))
  return (
    <div aria-hidden className="flex h-14 items-end gap-px">
      {moments.map((m, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: moments have no identity but their place
        <div key={i} className="flex min-w-0 flex-1 flex-col-reverse">
          {stack.map((t) =>
            (m[t] ?? 0) > 0 ? (
              <div
                key={t}
                className={classes("w-full", toneBg[t])}
                style={{ height: `${(56 * (m[t] ?? 0)) / top}px` }}
              />
            ) : null,
          )}
        </div>
      ))}
    </div>
  )
}

export function Download({
  title,
  doing,
  image,
  readings = [],
  moments = [],
  fraction,
  state,
  percent,
  action,
  compact = false,
  barTone = "origin",
}: DownloadProps) {
  const [failed, setFailed] = useState<string>()
  const art = image !== undefined && failed !== image ? image : undefined
  const pct = Math.min(100, Math.max(0, 100 * fraction))
  return (
    <section
      aria-label={`${doing} · ${title}`}
      className={classes(
        "relative isolate overflow-hidden rounded-xl border bg-sunken shadow-panel",
        compact ? "min-h-28" : "min-h-52",
      )}
    >
      {art !== undefined && (
        <>
          <img
            src={art}
            alt=""
            aria-hidden
            decoding="async"
            className="-z-20 absolute inset-0 size-full scale-110 object-cover opacity-50 blur-2xl"
          />
          <img
            src={art}
            alt=""
            decoding="async"
            onError={() => setFailed(art)}
            className={
              compact
                ? "hidden"
                : "-z-10 absolute inset-y-0 left-0 h-full w-auto max-w-[75%] object-cover object-left [mask-image:linear-gradient(to_right,#000_55%,transparent)]"
            }
          />
        </>
      )}
      <div aria-hidden className="-z-10 absolute inset-0 bg-linear-to-r from-transparent via-canvas/40 to-canvas/90" />
      <div
        aria-hidden
        className="-z-10 absolute inset-x-0 bottom-0 h-2/3 bg-linear-to-t from-canvas/85 to-transparent"
      />
      <div
        className={classes(
          "grid h-full gap-5 p-5",
          compact
            ? "md:grid-cols-[minmax(0,1fr)_minmax(16rem,24rem)]"
            : "md:grid-cols-[minmax(0,1fr)_minmax(18rem,30rem)]",
        )}
      >
        <div className="flex min-w-0 flex-col justify-end gap-1 [text-shadow:0_1px_12px_rgb(0_0_0/0.45)]">
          <span className="font-mono text-[11px] text-text/80 uppercase tracking-[0.16em]">{doing}</span>
          <h3 className={classes("truncate font-semibold tracking-tight", compact ? "text-xl" : "text-3xl")}>
            {title}
          </h3>
        </div>
        <div className="flex min-w-0 flex-col justify-end gap-3">
          {!compact && readings.length > 0 && (
            <dl className="flex flex-wrap gap-x-6 gap-y-2">
              {readings.map((r) => (
                <div key={r.label} className="flex flex-col">
                  <dt className="flex items-center gap-1.5 font-mono text-[10px] text-muted uppercase tracking-[0.14em]">
                    {r.tone !== undefined && (
                      <span aria-hidden className={classes("size-1.5 rounded-sm", toneBg[r.tone])} />
                    )}
                    {r.label}
                  </dt>
                  <dd className="font-mono text-sm tabular-nums">{r.value}</dd>
                </div>
              ))}
            </dl>
          )}
          {!compact && moments.length > 0 && <Graph moments={moments} />}
          <div className="flex flex-col gap-1.5">
            <div className="flex items-baseline justify-between gap-3 text-xs">
              <span className="truncate text-muted">{state}</span>
              <span className="font-mono font-semibold tabular-nums">{percent}</span>
            </div>
            <meter className="sr-only" min={0} max={100} value={Math.round(pct)} aria-label={`${title}: ${percent}`} />
            <div aria-hidden className="h-1.5 overflow-hidden rounded-full bg-text/10">
              <div
                className={classes("h-full rounded-full transition-[width] duration-700", toneBg[barTone])}
                style={{ width: `${pct}%` }}
              />
            </div>
          </div>
          {action !== undefined && <div className="flex justify-end gap-2">{action}</div>}
        </div>
      </div>
    </section>
  )
}
