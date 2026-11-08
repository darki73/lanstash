/**
 * Text: the few kinds of words beside the figures: muted words (a note), a link within them, a file's path cut to one line, and a quiet message
 * standing in for an empty list, with a line under it saying why.
 */
import type { ReactNode } from "react"

export function Muted({ children }: { readonly children?: ReactNode }) {
  return <span className="text-muted">{children}</span>
}

export interface QuietProps {
  readonly title: string
  readonly hint?: string
}

export function Quiet({ title, hint }: QuietProps) {
  return (
    <div className="flex w-full flex-col items-center gap-1 rounded-lg border border-dashed px-4 py-8 text-center">
      <span className="text-muted">{title}</span>
      {hint !== undefined && <span className="text-faint text-xs">{hint}</span>}
    </div>
  )
}

/** A link inside a panel's words (to another page of the app). */
export function Link({ href, children }: { readonly href: string; readonly children: string }) {
  return (
    <a
      href={href}
      className="w-fit rounded-sm text-cache text-sm underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cache"
    >
      {children}
    </a>
  )
}

/** A file's path on one line in the monospace face, cut short with an ellipsis (whole on hover). */
export function Path({ children }: { readonly children: string }) {
  return (
    <span title={children} className="block min-w-0 truncate font-mono text-[11px] text-faint">
      {children}
    </span>
  )
}
