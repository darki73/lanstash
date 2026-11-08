/**
 * Stack: what a panel shows, one part under the other with room between them (a code to scan,
 * a table, a line saying why something failed), each part at its own width; or, `wide`, each part
 * across the whole panel (a chart, the figures under it).
 */
import type { ReactNode } from "react"
import { classes } from "@/lib/classes"

export function Stack({ wide = false, children }: { readonly wide?: boolean; readonly children?: ReactNode }) {
  return (
    <div className={classes("flex flex-col gap-4 [&>*]:max-w-full", wide ? "items-stretch gap-6" : "items-start")}>
      {children}
    </div>
  )
}
