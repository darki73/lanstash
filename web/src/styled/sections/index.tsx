/**
 * Sections: a page split into parts, with their own navigation. On a wide screen the links stand in
 * a column on the left of the part shown, and stay in view while it scrolls; on a narrow one they are
 * a row above it that scrolls sideways. A link has its words, an optional figure on the right (a
 * count, a percentage) and an optional dot: amber while something runs there, red when something
 * there needs a look. The part shown is marked.
 */
import type { ReactNode } from "react"
import { classes } from "@/lib/classes"

export interface SectionsProps {
  /** What the links are for, for assistive technology ("Steam's sections"). */
  readonly label: string
  /** The links ([`SectionLink`]s). */
  readonly links: ReactNode
  readonly children?: ReactNode
}

export function Sections({ label, links, children }: SectionsProps) {
  return (
    <div className="flex flex-col gap-6 lg:grid lg:grid-cols-[13rem_minmax(0,1fr)] lg:items-start lg:gap-8">
      <nav
        aria-label={label}
        className="-mx-1 flex gap-1 overflow-x-auto px-1 pb-1 lg:sticky lg:top-24 lg:mx-0 lg:flex-col lg:overflow-visible lg:border-r lg:px-0 lg:pr-4 lg:pb-0"
      >
        {links}
      </nav>
      <div className="flex min-w-0 flex-col gap-6">{children}</div>
    </div>
  )
}

export type SectionDot = "busy" | "attention"

export interface SectionLinkProps {
  readonly href: string
  readonly current: boolean
  readonly label: string
  readonly figure?: string
  readonly dot?: SectionDot
}

export function SectionLink({ href, current, label, figure, dot }: SectionLinkProps) {
  return (
    <a
      href={href}
      aria-current={current ? "page" : undefined}
      className={classes(
        "flex shrink-0 items-center gap-2 rounded-md px-2.5 py-1.5 text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cache",
        current
          ? "bg-raised font-medium text-text shadow-panel ring-1 ring-line"
          : "text-muted hover:bg-hover hover:text-text",
      )}
    >
      {dot !== undefined && (
        <span
          role="img"
          aria-label={dot === "busy" ? "at work" : "needs a look"}
          className={classes("size-1.5 shrink-0 rounded-full", dot === "busy" ? "animate-pulse bg-cache" : "bg-danger")}
        />
      )}
      <span className="flex-1 truncate">{label}</span>
      {figure !== undefined && <span className="font-mono text-faint text-xs tabular-nums">{figure}</span>}
    </a>
  )
}
