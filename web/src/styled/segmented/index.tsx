/**
 * Segmented: a small switch between a few choices (such as how a list is sorted), one pressed at
 * a time, with a label for assistive technology.
 */
import { classes } from "@/lib/classes"

export interface SegmentedProps<T extends string> {
  readonly label: string
  readonly options: readonly { readonly id: T; readonly label: string }[]
  readonly value: T
  readonly onChange: (id: T) => void
}

export function Segmented<T extends string>({ label, options, value, onChange }: SegmentedProps<T>) {
  return (
    <fieldset className="inline-flex rounded-md border bg-sunken p-0.5 text-xs">
      <legend className="sr-only">{label}</legend>
      {options.map((o) => (
        <button
          key={o.id}
          type="button"
          aria-pressed={o.id === value}
          onClick={() => onChange(o.id)}
          className={classes(
            "cursor-pointer rounded px-2.5 py-1 transition-colors",
            o.id === value ? "bg-raised text-text shadow-panel" : "text-muted hover:text-text",
          )}
        >
          {o.label}
        </button>
      ))}
    </fieldset>
  )
}
