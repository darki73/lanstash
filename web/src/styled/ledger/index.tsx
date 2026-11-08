/**
 * Ledger: a band of what something did over its whole life, read at a glance. On the left a headline
 * figure, large in the monospace face, with a caption under it; in the middle a bar split into
 * shares (each in its tone, the share written beside its label); on the right rows, each an icon,
 * a label, a thin bar for its share of the largest and its figure, or, without rows, plain facts
 * (a label and its figure, with a dot in a tone when given). On a narrow screen the three stack.
 */
import type { ReactNode } from "react"
import { classes } from "@/lib/classes"
import { type Tone, toneBg } from "@/styled/tone"

export interface LedgerShare {
  readonly label: string
  readonly value: string
  /** 0..1 of the whole. */
  readonly share: number
  readonly tone: Tone
}

export interface LedgerRow {
  readonly key: string
  readonly icon?: ReactNode
  readonly label: string
  readonly value: string
  /** 0..1 of the largest row. */
  readonly share: number
}

export interface LedgerProps {
  /** What the band is about, for assistive technology. */
  readonly label: string
  readonly headline: string
  readonly caption: string
  readonly split: readonly LedgerShare[]
  /** Said over the rows. */
  readonly rowsTitle?: string
  readonly rows: readonly LedgerRow[]
  /** Plain facts in place of rows (none: the rows): a label on the left, its figure on the right. */
  readonly facts?: readonly LedgerFact[]
}

export interface LedgerFact {
  readonly label: string
  readonly value: string
  readonly tone?: Tone
}

const pct = (share: number) => `${Math.round(100 * Math.min(1, Math.max(0, share)))} %`

export function Ledger({ label, headline, caption, split, rowsTitle, rows, facts = [] }: LedgerProps) {
  return (
    <section
      aria-label={label}
      className="grid animate-rise gap-6 rounded-xl border bg-raised p-5 shadow-panel md:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)_minmax(0,1fr)] md:items-center"
    >
      <div className="flex min-w-0 flex-col gap-1">
        <span className="font-mono text-4xl text-cache tabular-nums tracking-tight">{headline}</span>
        <span className="text-muted text-sm leading-snug">{caption}</span>
      </div>
      <div className="flex min-w-0 flex-col gap-2">
        <div className="flex h-2.5 overflow-hidden rounded-full bg-sunken">
          {split.map((s) => (
            <div
              key={s.label}
              className={classes("h-full first:rounded-s-full last:rounded-e-full", toneBg[s.tone])}
              style={{ width: pct(s.share).replace(" ", "") }}
            />
          ))}
        </div>
        <dl className="flex flex-wrap gap-x-5 gap-y-1 text-xs">
          {split.map((s) => (
            <div key={s.label} className="flex items-center gap-1.5">
              <span aria-hidden className={classes("size-2 rounded-full", toneBg[s.tone])} />
              <dt className="text-muted">{s.label}</dt>
              <dd className="font-mono tabular-nums">{`${s.value} · ${pct(s.share)}`}</dd>
            </div>
          ))}
        </dl>
      </div>
      {rows.length > 0 && (
        <div className="flex min-w-0 flex-col gap-2">
          {rowsTitle !== undefined && (
            <span className="font-mono text-[10px] text-faint uppercase tracking-[0.14em]">{rowsTitle}</span>
          )}
          <ul className="flex flex-col gap-2">
            {rows.map((r) => (
              <li key={r.key} className="flex items-center gap-2.5 text-sm">
                {r.icon}
                <span className="w-24 shrink-0 truncate">{r.label}</span>
                <span className="h-1 flex-1 overflow-hidden rounded-full bg-sunken">
                  <span
                    className="block h-full rounded-full bg-cache/70"
                    style={{ width: pct(r.share).replace(" ", "") }}
                  />
                </span>
                <span className="w-20 shrink-0 text-end font-mono text-muted text-xs tabular-nums">{r.value}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
      {rows.length === 0 && facts.length > 0 && (
        <dl className="flex min-w-0 flex-col gap-1.5 text-sm">
          {facts.map((f) => (
            <div
              key={f.label}
              className="flex items-baseline justify-between gap-4 border-b border-dashed pb-1.5 last:border-b-0"
            >
              <dt className="flex items-center gap-1.5 text-muted">
                {f.tone !== undefined && (
                  <span aria-hidden className={classes("size-2 rounded-full", toneBg[f.tone])} />
                )}
                {f.label}
              </dt>
              <dd className="font-mono tabular-nums">{f.value}</dd>
            </div>
          ))}
        </dl>
      )}
    </section>
  )
}
