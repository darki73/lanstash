/**
 * Checklist: things checked, one row each, read from the start: a badge saying how it came out
 * (a word, never the colour alone), what was checked, and the detail, which wraps rather than runs
 * off. On a narrow screen the detail goes under the name.
 */
import type { ReactNode } from "react"

export interface ChecklistItem {
  readonly key: string
  /** How it came out (a Signal). */
  readonly badge: ReactNode
  readonly title: string
  readonly detail: string
}

export function Checklist({ label, items }: { readonly label: string; readonly items: readonly ChecklistItem[] }) {
  return (
    <ul aria-label={label} className="flex flex-col">
      {items.map((i) => (
        <li
          key={i.key}
          className="grid grid-cols-[6.5rem_minmax(0,1fr)] items-baseline gap-x-4 gap-y-1 border-b py-3 last:border-b-0 md:grid-cols-[6.5rem_11rem_minmax(0,1fr)]"
        >
          <span className="self-center">{i.badge}</span>
          <span className="font-medium text-sm">{i.title}</span>
          <span className="col-start-2 text-muted text-sm leading-snug md:col-start-3">{i.detail}</span>
        </li>
      ))}
    </ul>
  )
}
