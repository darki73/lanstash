/**
 * Toolbar: the line over a list: its title on the left, its controls (finding, filters, sorting) on
 * the right, side by side, onto more lines where they do not fit.
 */
import type { ReactNode } from "react"

export interface ToolbarProps {
  readonly title: string
  readonly tools?: ReactNode
}

export function Toolbar({ title, tools }: ToolbarProps) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <h3 className="font-medium">{title}</h3>
      {tools !== undefined && <div className="flex flex-wrap items-center gap-2">{tools}</div>}
    </div>
  )
}
