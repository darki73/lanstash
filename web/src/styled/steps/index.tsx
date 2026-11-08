/**
 * Steps: what to do, in order: each step a number in a ring on a line that joins it to the next,
 * its title, and what it says under it.
 * Runs: who runs each part of a step, in order: a pill naming who, its word in the middle (user,
 * or administrator in the warning colour: it may change the system), beside what that part does.
 * FileCard: a file to download: a page glyph, its name in the monospace face, a line about it, and
 * a Download button (disabled where the file is not there to download yet).
 * Command: a few lines to copy (a script, a file's lines) in the monospace face on a sunken block,
 * with a Copy button over its corner that says Copied for a moment once it did.
 */
import { type ReactNode, useState } from "react"
import { useT } from "@/i18n"
import { classes } from "@/lib/classes"
import { Button } from "@/styled/button"

export interface StepsProps {
  /** What the steps are for, for assistive technology. */
  readonly label: string
  readonly children?: ReactNode
}

export function Steps({ label, children }: StepsProps) {
  return (
    <ol aria-label={label} className="flex flex-col">
      {children}
    </ol>
  )
}

export interface StepProps {
  readonly n: number
  readonly title: string
  /** The last step: no line on to a next. */
  readonly last?: boolean
  readonly children?: ReactNode
}

export function Step({ n, title, last = false, children }: StepProps) {
  return (
    <li className="grid grid-cols-[2rem_minmax(0,1fr)] gap-x-4">
      <span className="flex flex-col items-center">
        <span
          aria-hidden
          className="flex size-8 shrink-0 items-center justify-center rounded-full border border-cache/50 bg-cache/12 font-mono font-semibold text-cache text-xs tabular-nums"
        >
          {n}
        </span>
        {!last && <span aria-hidden className="my-1.5 w-px flex-1 bg-line" />}
      </span>
      <div className={classes("flex min-w-0 flex-col gap-2.5 pt-1.5", !last && "pb-7")}>
        <span className="font-semibold text-[15px] text-text">{title}</span>
        {children !== undefined && (
          <div className="flex flex-col items-start gap-2.5 text-[13.5px] text-muted leading-relaxed">{children}</div>
        )}
      </div>
    </li>
  )
}

export interface Run {
  readonly who: "user" | "administrator"
  readonly does: string
  /** Shown under what it does (the lines it writes). */
  readonly detail?: ReactNode
}

export function Runs({ label, runs }: { readonly label: string; readonly runs: readonly Run[] }) {
  const t = useT("common")
  return (
    <ol aria-label={label} className="flex w-full flex-col overflow-hidden rounded-lg border">
      {runs.map((r, i) => (
        <li
          key={r.does}
          className={classes(
            "grid grid-cols-1 gap-x-4 gap-y-2 px-3.5 py-3 sm:grid-cols-[8rem_minmax(0,1fr)] sm:items-start",
            i > 0 && "border-t",
            r.who === "administrator" ? "bg-warning/6" : "bg-raised",
          )}
        >
          <span
            className={classes(
              "inline-flex h-6 w-fit items-center justify-center whitespace-nowrap rounded-full border px-3 font-medium text-xs sm:w-full",
              r.who === "administrator"
                ? "border-warning/50 bg-warning/15 text-text"
                : "border-success/40 bg-success/10 text-text",
            )}
          >
            {t(`ui.who.${r.who}`)}
          </span>
          <span className="flex min-w-0 flex-col gap-2 text-text leading-6">
            {r.does}
            {r.detail}
          </span>
        </li>
      ))}
    </ol>
  )
}

export interface FileCardProps {
  readonly name: string
  readonly note: string
  /** The file's address, saved under its name; the button is disabled without it. */
  readonly href?: string
}

export function FileCard({ name, note, href }: FileCardProps) {
  const t = useT("common")
  return (
    <div className="flex w-full max-w-xl items-center gap-3.5 rounded-lg border bg-raised px-3.5 py-3 shadow-panel">
      <span
        aria-hidden
        className="flex size-10 shrink-0 items-center justify-center rounded-md border border-cache/40 bg-cache/10 text-cache"
      >
        <svg aria-hidden viewBox="0 0 24 24" className="size-5" fill="none" stroke="currentColor" strokeWidth="1.6">
          <path d="M7 3h7l4 4v14H7z" />
          <path d="M14 3v4h4M10 12h5M10 15.5h5" />
        </svg>
      </span>
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate font-mono text-[13px] text-text">{name}</span>
        <span className="text-muted text-xs">{note}</span>
      </span>
      {href === undefined ? (
        <Button onClick={() => {}} tone="accent" disabled label={t("ui.downloadName", { name })}>
          {t("ui.download")}
        </Button>
      ) : (
        <a
          href={href}
          download={name}
          aria-label={t("ui.downloadName", { name })}
          className="whitespace-nowrap rounded-md border border-origin/40 bg-origin/15 px-2.5 py-1 font-medium text-origin text-xs transition-colors hover:bg-origin/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
        >
          {t("ui.download")}
        </a>
      )}
    </div>
  )
}

export interface CommandProps {
  /** What the lines are, for the Copy button's label ("the lines for Engine.ini"). */
  readonly label: string
  readonly text: string
}

export function Command({ label, text }: CommandProps) {
  const t = useT("common")
  const [copied, setCopied] = useState(false)
  const copy = () => {
    void navigator.clipboard?.writeText(text).then(
      () => {
        setCopied(true)
        setTimeout(() => setCopied(false), 1500)
      },
      () => {},
    )
  }
  return (
    <div className="relative self-stretch">
      <pre className="overflow-x-auto rounded-lg border bg-sunken py-3 ps-3.5 pe-20 font-mono text-[12.5px] text-text leading-relaxed">
        {text}
      </pre>
      <div className="absolute end-2 top-2">
        <Button onClick={copy} label={t("ui.copyWhat", { what: label })}>
          {copied ? t("ui.copied") : t("ui.copy")}
        </Button>
      </div>
    </div>
  )
}
