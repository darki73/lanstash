/**
 * Chips: a row of small facts under a page's title, each a figure with its label after it
 * ("46.43 GB stored", "4 games"), in hairline pills; one may carry a tone swatch.
 */
import type { ReactNode } from "react"
import { classes } from "@/lib/classes"
import { type Tone, toneBg } from "@/styled/tone"

export function Chips({ children }: { readonly children?: ReactNode }) {
  return <ul className="flex flex-wrap gap-2">{children}</ul>
}

export interface ChipProps {
  readonly value: string
  readonly label: string
  readonly tone?: Tone
}

export function Chip({ value, label, tone }: ChipProps) {
  return (
    <li className="inline-flex items-center gap-1.5 rounded-full border bg-raised px-3 py-1 text-xs shadow-panel">
      {tone !== undefined && <span aria-hidden className={classes("size-1.5 rounded-full", toneBg[tone])} />}
      <span className="font-mono tabular-nums">{value}</span>
      <span className="text-muted">{label}</span>
    </li>
  )
}
