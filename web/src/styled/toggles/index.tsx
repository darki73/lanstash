/**
 * Toggles: several of a few things chosen at once, each a pill that is pressed or not (the platform
 * accounts linked to a lanstash account). A pressed pill is lit in the cache's amber with a tick; one
 * not pressed is a quiet dashed outline. A pill may name its group first, faint ("steam / someone").
 * With nothing to choose from, a quiet line says why.
 */
import { classes } from "@/lib/classes"

export interface ToggleOption {
  readonly id: string
  readonly label: string
  /** What it belongs to, written faint before the label (its platform). */
  readonly group?: string
}

export interface TogglesProps {
  readonly label: string
  readonly options: readonly ToggleOption[]
  readonly values: readonly string[]
  readonly onChange: (values: readonly string[]) => void
  /** What is said when there is nothing to choose from. */
  readonly empty?: string
}

export function Toggles({ label, options, values, onChange, empty }: TogglesProps) {
  if (options.length === 0) {
    return empty === undefined ? null : <span className="text-faint text-xs">{empty}</span>
  }
  return (
    <fieldset className="flex flex-wrap items-center gap-1.5">
      <legend className="sr-only">{label}</legend>
      {options.map((o) => {
        const on = values.includes(o.id)
        return (
          <button
            key={o.id}
            type="button"
            aria-pressed={on}
            aria-label={`${o.group === undefined ? "" : `${o.group} `}${o.label}`}
            onClick={() => onChange(on ? values.filter((v) => v !== o.id) : [...values, o.id])}
            className={classes(
              "inline-flex cursor-pointer items-center gap-1.5 rounded-full border px-2.5 py-1 font-mono text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
              on
                ? "border-cache/50 bg-cache/12 text-text hover:bg-cache/20"
                : "border-dashed text-muted hover:border-solid hover:bg-hover hover:text-text",
            )}
          >
            <span
              aria-hidden
              className={classes(
                "inline-flex size-3.5 items-center justify-center rounded-full text-[9px] leading-none",
                on ? "bg-cache text-canvas" : "border border-current opacity-50",
              )}
            >
              {on ? "✓" : ""}
            </span>
            {o.group !== undefined && <span className="text-faint">{o.group} /</span>}
            {o.label}
          </button>
        )
      })}
    </fieldset>
  )
}
