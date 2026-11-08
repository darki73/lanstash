/**
 * Search: a one-line field that filters a list as it is typed, with a label for assistive
 * technology and a way to clear it (Escape, or the browser's own clear button). It fills its line
 * up to a width, or keeps to a narrow one beside other tools (compact).
 */
import { classes } from "@/lib/classes"

export interface SearchProps {
  readonly label: string
  readonly value: string
  readonly onChange: (value: string) => void
  readonly placeholder?: string
  /** Narrow, to sit in a toolbar beside other tools. */
  readonly compact?: boolean
}

export function Search({ label, value, onChange, placeholder, compact = false }: SearchProps) {
  return (
    <input
      type="search"
      aria-label={label}
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Escape") onChange("")
      }}
      className={classes(
        "rounded-md border bg-sunken px-3 py-1.5 text-sm text-text outline-none placeholder:text-faint focus:border-accent",
        compact ? "w-48" : "w-full max-w-sm",
      )}
    />
  )
}
