/**
 * Signal: a pill with a coloured dot and the word that says what it means (the colour is never
 * alone): good (the dot beats, like a heartbeat, unless still), waiting or bad; a title says more on hover.
 */
import { classes } from "@/lib/classes"

export type SignalTone = "good" | "waiting" | "bad"

export interface SignalProps {
  readonly tone: SignalTone
  readonly children: string
  /** A good dot that does not beat: for signals in lists, where many would. */
  readonly still?: boolean
  /** More than the word says, on hover; the word itself otherwise. */
  readonly title?: string
}

export function Signal({ tone, children, still = false, title }: SignalProps) {
  return (
    <span
      title={title ?? children}
      className="inline-flex min-w-0 max-w-full items-center gap-2 whitespace-nowrap rounded-full border bg-raised px-2.5 py-0.5 font-mono text-xs"
    >
      <span
        aria-hidden
        className={classes(
          "size-1.5 rounded-full",
          tone === "good" && "bg-success",
          tone === "good" && !still && "animate-beat",
          tone === "waiting" && "bg-warning",
          tone === "bad" && "bg-danger",
          "shrink-0",
        )}
      />
      <span className="truncate">{children}</span>
    </span>
  )
}
