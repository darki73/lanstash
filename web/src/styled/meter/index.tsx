/**
 * Meter: how far something has come, as a thin bar with its label beside it; the label is the
 * percentage unless one is given, kept whole on one line (the bar gives way). The fraction is clamped to 0..1. Assistive technology reads a
 * native meter element; the bar drawn over it is hidden from it.
 */
import { fraction as clamp } from "@/lib/clamp"

export interface MeterProps {
  /** 0 to 1. */
  readonly fraction: number
  readonly label?: string
}

export function Meter({ fraction, label }: MeterProps) {
  const pct = 100 * clamp(fraction)
  const text = label ?? `${pct.toFixed(0)} %`
  return (
    <div className="flex min-w-36 items-center gap-2.5">
      <meter className="sr-only" min={0} max={100} value={Math.round(pct)} aria-label={text} />
      <div aria-hidden className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-sunken">
        <div className="h-full rounded-full bg-origin transition-[width] duration-700" style={{ width: `${pct}%` }} />
      </div>
      <span aria-hidden className="min-w-16 shrink-0 whitespace-nowrap text-end font-mono text-xs tabular-nums">
        {text}
      </span>
    </div>
  )
}
