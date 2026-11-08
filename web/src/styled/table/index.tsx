/**
 * Table: rows of figures. The first column is words and reads from the start; the rest are
 * figures and line up at the end, in the monospace face. A row is plain, strong (a total over the
 * rows under it) or quiet (a detail of the row above); a cell can be indented (it belongs to the
 * strong row above), set as a path (small monospace, broken anywhere) or muted.
 */
import type { ReactNode } from "react"
import { classes } from "@/lib/classes"

export interface TableProps {
  /** The column heads, each different (a head is its column's identity). */
  readonly columns: readonly string[]
  readonly children?: ReactNode
}

export function Table({ columns, children }: TableProps) {
  return (
    <table className="w-full border-collapse font-mono text-xs tabular-nums">
      <thead>
        <tr>
          {columns.map((c) => (
            <th
              key={c}
              className="whitespace-nowrap border-b px-2 pb-2 text-end font-normal text-[11px] text-faint uppercase tracking-[0.12em] first:ps-0 first:text-start"
            >
              {c}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>{children}</tbody>
    </table>
  )
}

export type RowTone = "plain" | "strong" | "quiet"

export function Row({ tone = "plain", children }: { readonly tone?: RowTone; readonly children?: ReactNode }) {
  return (
    <tr
      className={classes(
        "transition-colors hover:bg-hover",
        tone === "strong" && "font-semibold",
        tone === "quiet" && "text-muted [&>td]:border-dashed",
      )}
    >
      {children}
    </tr>
  )
}

export interface CellProps {
  readonly indent?: boolean
  readonly path?: boolean
  readonly muted?: boolean
  readonly children?: ReactNode
}

export function Cell({ indent, path, muted, children }: CellProps) {
  return (
    <td
      className={classes(
        "whitespace-nowrap border-b px-2 py-2 text-end align-middle first:ps-0 first:text-start",
        indent && "first:ps-5",
        path && "whitespace-normal break-all text-[11px] text-muted",
        muted && "text-faint",
      )}
    >
      {children}
    </td>
  )
}
