/**
 * Capacity: how full a store is, as one bar across: a part per holder (a platform), each in the
 * cache's amber, the largest strongest and each next one fainter, then what is free left as the
 * bar's track; a figure for the whole over it, and under it a legend: each holder's mark, its name
 * and its figure. Shares are of the whole bar (0..1, clamped).
 */
import type { ReactNode } from "react"
import { fraction as clamp } from "@/lib/clamp"

export interface CapacityPart {
  readonly key: string
  readonly icon?: ReactNode
  readonly label: string
  readonly value: string
  /** 0..1 of the whole bar. */
  readonly share: number
}

export interface CapacityProps {
  /** What the bar is, for assistive technology. */
  readonly label: string
  /** The figure over the bar on the left (what is held) and on the right (what is free). */
  readonly held: string
  readonly free?: string
  readonly parts: readonly CapacityPart[]
}

const strengths = [1, 0.7, 0.48, 0.32, 0.22] as const

export function Capacity({ label, held, free, parts }: CapacityProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex items-baseline justify-between gap-4">
        <span className="font-mono text-2xl tabular-nums">{held}</span>
        {free !== undefined && <span className="font-mono text-muted text-sm tabular-nums">{free}</span>}
      </div>
      <div role="img" aria-label={label} className="flex h-3 overflow-hidden rounded-full bg-sunken">
        {parts.map((p, i) => (
          <div
            key={p.key}
            className="h-full bg-cache first:rounded-s-full"
            style={{
              width: `${100 * clamp(p.share)}%`,
              opacity: strengths[Math.min(i, strengths.length - 1)],
            }}
          />
        ))}
      </div>
      <ul className="flex flex-wrap gap-x-5 gap-y-1.5 text-sm">
        {parts.map((p, i) => (
          <li key={p.key} className="flex items-center gap-2">
            <span
              aria-hidden
              className="size-2 rounded-full bg-cache"
              style={{ opacity: strengths[Math.min(i, strengths.length - 1)] }}
            />
            {p.icon}
            <span>{p.label}</span>
            <span className="font-mono text-muted text-xs tabular-nums">{p.value}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}
