/**
 * Readings: a dense list of named figures, a name on the left and its figure on the right, with
 * an optional note after the figure (a rate beside a byte count). For the raw counters of a view.
 */

export interface Reading {
  readonly name: string
  readonly value: string
  readonly note?: string
}

export function Readings({ rows }: { readonly rows: readonly Reading[] }) {
  return (
    <dl className="flex flex-col font-mono text-xs tabular-nums">
      {rows.map((r) => (
        <div key={r.name} className="flex items-baseline gap-3 border-b border-dashed py-1 last:border-b-0">
          <dt className="min-w-0 flex-1 truncate font-sans text-muted">{r.name}</dt>
          <dd className="text-end">{r.value}</dd>
          <dd className="w-20 text-end text-faint">{r.note ?? ""}</dd>
        </div>
      ))}
    </dl>
  )
}
