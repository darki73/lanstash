/**
 * Button: a small action. accent is the one action that starts something (in the origin colour:
 * it fetches from the internet), danger the confirmation of one that deletes, plain everything else.
 * Disabled while it cannot act. OutLink: a link that looks like a button and opens another site's
 * page in a new tab (a sign-in page), saying so with an arrow.
 */
import { classes } from "@/lib/classes"

export interface ButtonProps {
  readonly children: string
  readonly onClick: () => void
  readonly tone?: "accent" | "plain" | "danger"
  readonly disabled?: boolean
  /** What it does, for assistive technology, when the words alone do not say. */
  readonly label?: string
}

const look = (tone: "accent" | "plain" | "danger") =>
  classes(
    "cursor-pointer whitespace-nowrap rounded-md border px-2.5 py-1 font-medium text-xs transition-colors disabled:cursor-not-allowed disabled:opacity-40",
    tone === "accent" && "border-origin/40 bg-origin/15 text-origin hover:bg-origin/25",
    tone === "danger" && "border-danger/40 bg-danger/15 text-danger hover:bg-danger/25",
    tone === "plain" && "bg-sunken text-muted hover:text-text",
  )

export function Button({ children, onClick, tone = "plain", disabled = false, label }: ButtonProps) {
  return (
    <button type="button" onClick={onClick} disabled={disabled} aria-label={label} className={look(tone)}>
      {children}
    </button>
  )
}

export interface OutLinkProps {
  readonly href: string
  readonly children: string
  readonly tone?: "accent" | "plain"
}

export function OutLink({ href, children, tone = "accent" }: OutLinkProps) {
  return (
    <a href={href} target="_blank" rel="noopener noreferrer" className={classes(look(tone), "inline-block")}>
      {`${children} ↗`}
    </a>
  )
}
