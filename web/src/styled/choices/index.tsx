/**
 * Choices: a few large choices side by side (as many to a row as fit), one pressed at a time: each
 * its name and a line under it saying where it stands, after a dot in its tone (good, waiting,
 * bad; the words always say it too). The pressed one is raised and ringed in the cache's amber.
 */
import { classes } from "@/lib/classes"
import type { SignalTone } from "@/styled/signal"

export interface Choice<T extends string> {
  readonly id: T
  readonly label: string
  readonly note: string
  readonly tone: SignalTone
}

export interface ChoicesProps<T extends string> {
  /** What is chosen, for assistive technology. */
  readonly label: string
  readonly options: readonly Choice<T>[]
  readonly value: T
  readonly onChange: (id: T) => void
}

const dot: Readonly<Record<SignalTone, string>> = {
  good: "bg-success",
  waiting: "bg-warning",
  bad: "bg-danger",
}

export function Choices<T extends string>({ label, options, value, onChange }: ChoicesProps<T>) {
  return (
    <fieldset className="grid grid-cols-[repeat(auto-fit,minmax(10rem,1fr))] gap-2">
      <legend className="sr-only">{label}</legend>
      {options.map((o) => (
        <button
          key={o.id}
          type="button"
          aria-pressed={o.id === value}
          onClick={() => onChange(o.id)}
          className={classes(
            "flex cursor-pointer flex-col items-start gap-1 rounded-lg border px-3.5 py-2.5 text-start transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
            o.id === value
              ? "border-cache/60 bg-raised shadow-panel ring-1 ring-cache/30"
              : "bg-sunken/60 hover:border-cache/30 hover:bg-raised",
          )}
        >
          <span className="font-medium text-sm text-text">{o.label}</span>
          <span className="flex items-center gap-1.5 text-muted text-xs">
            <span aria-hidden className={classes("size-1.5 shrink-0 rounded-full", dot[o.tone])} />
            {o.note}
          </span>
        </button>
      ))}
    </fieldset>
  )
}
