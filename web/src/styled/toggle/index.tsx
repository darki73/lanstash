/**
 * Toggle: a small button that opens and closes what is under it, with an arrow that says which
 * it is about to do.
 */

export interface ToggleProps {
  readonly open: boolean
  readonly onToggle: () => void
  readonly children: string
}

export function Toggle({ open, onToggle, children }: ToggleProps) {
  return (
    <button
      type="button"
      aria-expanded={open}
      onClick={onToggle}
      className="cursor-pointer whitespace-nowrap rounded-sm px-1 text-accent hover:bg-hover"
    >
      {open ? "▾" : "▸"} {children}
    </button>
  )
}
