/**
 * StatusLine: the line over a page that says, at a glance, what is going on: a dot and the words
 * for it, large (busy: the dot beats; idle: it is faint; trouble: it is red), then a few facts
 * after it, small and muted, kept apart by dots, and an action on the right (such as a switch of
 * the span the page shows). On a narrow screen the action wraps under the words.
 */
import { Fragment, type ReactNode } from "react"
import { useT } from "@/i18n"
import { classes } from "@/lib/classes"

export type StatusMood = "busy" | "idle" | "trouble"

export interface StatusLineProps {
  readonly mood: StatusMood
  readonly words: string
  /** Small facts after the words, in order. */
  readonly facts?: readonly ReactNode[]
  readonly action?: ReactNode
}

export function StatusLine({ mood, words, facts = [], action }: StatusLineProps) {
  const t = useT("common")
  return (
    <section
      aria-label={t("ui.status")}
      className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 rounded-xl border bg-raised px-4 py-3 shadow-panel"
    >
      <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-1">
        <span className="flex items-center gap-2.5">
          <span
            aria-hidden
            className={classes(
              "size-2 shrink-0 rounded-full",
              mood === "busy" && "animate-beat bg-success",
              mood === "idle" && "bg-faint",
              mood === "trouble" && "bg-danger",
            )}
          />
          <span className="font-medium text-base">{words}</span>
        </span>
        {facts.length > 0 && (
          <span className="flex flex-wrap items-center gap-x-2 gap-y-1 text-muted text-sm">
            {facts.map((f, i) => (
              // Facts are fixed in order per render, so their place is their identity.
              // biome-ignore lint/suspicious/noArrayIndexKey: see above
              <Fragment key={i}>
                {i > 0 && (
                  <span aria-hidden className="text-faint">
                    ·
                  </span>
                )}
                <span>{f}</span>
              </Fragment>
            ))}
          </span>
        )}
      </div>
      {action}
    </section>
  )
}
