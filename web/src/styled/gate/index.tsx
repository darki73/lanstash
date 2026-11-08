/**
 * Gate: the sign-in card, alone in the middle of the page. On top lanstash's mark, a title and a
 * line under it; then a form (Enter sends it, and password managers know it as a sign-in): each
 * field's label over its box (a fixed one dashed and muted, to read only), the boxes across the card, one wide button, and what went wrong
 * under it in the danger colour; under the card a quiet hint.
 */
import type { ReactNode } from "react"
import { classes } from "@/lib/classes"

export interface GateField {
  readonly label: string
  readonly value: string
  readonly onChange: (value: string) => void
  /** username: the name; current-password and new-password: what is typed is hidden (a new one
   * is offered to the password manager to keep). */
  readonly kind: "username" | "current-password" | "new-password"
  /** Shown but not to be changed (a name chosen for you). */
  readonly fixed?: boolean
}

export interface GateProps {
  readonly title: string
  readonly subtitle: string
  readonly fields: readonly GateField[]
  readonly submit: string
  readonly onSubmit: () => void
  readonly busy?: boolean
  readonly error?: string
  readonly hint?: ReactNode
}

function Mark() {
  return (
    <svg aria-hidden viewBox="0 0 24 24" className="size-10">
      <rect x="3" y="3.5" width="18" height="4.5" rx="1.5" className="fill-cache" />
      <rect x="3" y="9.75" width="18" height="4.5" rx="1.5" className="fill-text" opacity="0.55" />
      <rect x="3" y="16" width="18" height="4.5" rx="1.5" className="fill-text" opacity="0.28" />
    </svg>
  )
}

export function Gate({ title, subtitle, fields, submit, onSubmit, busy = false, error, hint }: GateProps) {
  const ready = !busy && fields.every((f) => f.value !== "")
  return (
    <div className="flex min-h-[70vh] animate-rise items-center justify-center py-10">
      <div className="flex w-full max-w-sm flex-col gap-5">
        <section aria-label={title} className="flex flex-col gap-6 rounded-2xl border bg-raised p-8 shadow-panel">
          <div className="flex flex-col items-center gap-3 text-center">
            <Mark />
            <div className="flex flex-col gap-1">
              <h2 className="font-semibold text-xl tracking-tight">{title}</h2>
              <p className="text-muted text-sm">{subtitle}</p>
            </div>
          </div>
          <form
            className="flex flex-col gap-4"
            onSubmit={(e) => {
              e.preventDefault()
              if (ready) onSubmit()
            }}
          >
            {fields.map((f) => (
              <label key={f.label} className="flex flex-col gap-1.5">
                <span className="font-medium text-muted text-xs">{f.label}</span>
                <input
                  type={f.kind === "username" ? "text" : "password"}
                  name={f.kind}
                  autoComplete={f.kind}
                  spellCheck={false}
                  autoCapitalize="none"
                  value={f.value}
                  readOnly={f.fixed}
                  onChange={(e) => f.onChange(e.target.value)}
                  className={classes(
                    "w-full rounded-lg border px-3 py-2.5 text-sm outline-none transition-colors placeholder:text-faint focus:border-accent",
                    f.fixed
                      ? "cursor-default border-dashed bg-transparent font-mono text-muted"
                      : "bg-sunken text-text",
                  )}
                />
              </label>
            ))}
            {error !== undefined && (
              <p role="alert" className="rounded-lg border border-danger/40 bg-danger/10 px-3 py-2 text-danger text-sm">
                {error}
              </p>
            )}
            <button
              type="submit"
              disabled={!ready}
              className={classes(
                "mt-1 w-full rounded-lg px-3 py-2.5 font-medium text-sm transition-colors",
                ready
                  ? "cursor-pointer bg-cache text-canvas hover:bg-cache/90"
                  : "cursor-not-allowed bg-sunken text-faint",
              )}
            >
              {submit}
            </button>
          </form>
        </section>
        {hint !== undefined && <div className="px-2 text-center text-faint text-xs leading-relaxed">{hint}</div>}
      </div>
    </div>
  )
}
