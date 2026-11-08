/**
 * Figures: the readings under a chart, the way a system monitor lists them: a small label over a
 * figure in the monospace face, as many to a row as fit, under an optional heading. A figure may
 * carry a swatch in a tone (it is what that trace shows) and be large (the readings that matter
 * most). A figure that says nothing happened (a zero) is quiet, drawn faint so the ones that say
 * something stand out; one that counts trouble (failures, errors) above zero is an alarm, drawn in
 * the danger colour beside its label.
 */
import type { ReactNode } from "react"
import { classes } from "@/lib/classes"
import { type Tone, toneBg } from "@/styled/tone"

export interface FiguresProps {
  /** Said over the figures in small spaced capitals, such as the span they cover. */
  readonly title?: string
  readonly children?: ReactNode
}

export function Figures({ title, children }: FiguresProps) {
  const grid = <dl className="grid grid-cols-2 gap-x-6 gap-y-4 md:grid-cols-4">{children}</dl>
  if (title === undefined) return grid
  return (
    <div className="flex flex-col gap-3">
      <span className="font-mono text-[11px] text-faint uppercase tracking-[0.14em]">{title}</span>
      {grid}
    </div>
  )
}

export interface FigureProps {
  readonly label: string
  readonly value: string
  readonly tone?: Tone
  readonly large?: boolean
  /** The trace is dashed: the swatch says so. */
  readonly dashed?: boolean
  /** quiet: nothing happened (drawn faint); alarm: trouble counted (drawn in the danger colour). */
  readonly state?: "quiet" | "alarm"
}

export function Figure({ label, value, tone, large, dashed, state }: FigureProps) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="flex items-center gap-1.5 text-faint text-xs">
        {tone !== undefined && (
          <span
            aria-hidden
            className={classes(
              "inline-block h-0.5 w-3 rounded-full",
              toneBg[tone],
              dashed && "[mask-image:repeating-linear-gradient(90deg,#000_0_3px,transparent_3px_5px)]",
            )}
          />
        )}
        {label}
      </dt>
      <dd
        className={classes(
          "font-mono tabular-nums",
          large ? "text-2xl" : "text-base",
          state === "quiet" && "text-faint",
          state === "alarm" && "text-danger",
        )}
      >
        {value}
      </dd>
    </div>
  )
}
