/**
 * Portion: one figure and how it divides: the figure large in the monospace face with a caption
 * under it, a bar split into its parts (each in its tone, at its share of the whole), and under the
 * bar each part's dot, label and figure, one to a line. For a summary beside a list.
 */
import { fraction as clamp } from "@/lib/clamp"
import { classes } from "@/lib/classes"
import { type Tone, toneBg } from "@/styled/tone"

export interface PortionPart {
  readonly label: string
  readonly value: string
  /** 0..1 of the whole. */
  readonly share: number
  readonly tone: Tone
}

export interface PortionProps {
  readonly value: string
  readonly caption: string
  readonly parts: readonly PortionPart[]
}

export function Portion({ value, caption, parts }: PortionProps) {
  return (
    <div className="flex min-w-0 flex-col gap-3">
      <div className="flex flex-col">
        <span className="font-mono text-3xl tabular-nums tracking-tight">{value}</span>
        <span className="text-muted text-sm">{caption}</span>
      </div>
      <div aria-hidden className="flex h-2 overflow-hidden rounded-full bg-sunken">
        {parts.map((p) => (
          <div
            key={p.label}
            className={classes("h-full first:rounded-s-full last:rounded-e-full", toneBg[p.tone])}
            style={{ width: `${100 * clamp(p.share)}%` }}
          />
        ))}
      </div>
      <dl className="flex flex-col gap-1 text-sm">
        {parts.map((p) => (
          <div key={p.label} className="flex items-baseline justify-between gap-3">
            <dt className="flex items-center gap-2 whitespace-nowrap text-muted">
              <span aria-hidden className={classes("size-2 rounded-full", toneBg[p.tone])} />
              {p.label}
            </dt>
            <dd className="whitespace-nowrap font-mono tabular-nums">{p.value}</dd>
          </div>
        ))}
      </dl>
    </div>
  )
}
