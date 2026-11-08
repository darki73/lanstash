/**
 * Split: a page's parts in two columns where the space is wide: the main ones on the left, the side
 * ones (such as storage and upkeep) in a narrower column on the right; one column, the side ones
 * last, where it is narrow. A column whose parts all draw nothing (a reader may not delete) takes no
 * room: the other fills the row.
 */
import type { ReactNode } from "react"

export interface SplitProps {
  readonly side?: ReactNode
  readonly children?: ReactNode
}

export function Split({ side, children }: SplitProps) {
  return (
    <div className="flex flex-col gap-6 xl:grid xl:grid-cols-[minmax(0,1fr)_22rem] xl:items-start xl:has-[>:empty]:grid-cols-1">
      <div className="flex min-w-0 flex-col gap-6 empty:hidden">{children}</div>
      {side !== undefined && <div className="flex min-w-0 flex-col gap-6 empty:hidden">{side}</div>}
    </div>
  )
}
