/**
 * Roster: people (lanstash's accounts, the invites waiting for one) as a list of rows on one card,
 * split by hairlines. A row: a round avatar in a hue of the name's own with its initials (or a mark
 * of its own), the name with a small tag after it ("you") and a muted line under it; then what is
 * set on it (its platform accounts), and at the end its role and actions. On a narrow screen the
 * parts wrap under the name.
 *   - RoleSwitch: a role chosen among a few, the pressed one in the role's own tone.
 *   - RoleBadge: a role as a small pill in its tone.
 *   - Avatar: the round avatar on its own (the sidebar's account).
 */
import type { ReactNode } from "react"
import { classes } from "@/lib/classes"
import { hueOf, initialsOf } from "@/styled/cards"

/** A role's tone: the strongest (an admin) in the cache's amber, a middle one green, the least muted. */
export type RoleTone = "strong" | "middle" | "least"

const pressed: Readonly<Record<RoleTone, string>> = {
  strong: "bg-cache/15 text-cache ring-1 ring-cache/40",
  middle: "bg-success/15 text-success ring-1 ring-success/40",
  least: "bg-raised text-text ring-1 ring-line",
}

export function Roster({ label, children }: { readonly label: string; readonly children?: ReactNode }) {
  return (
    <ul aria-label={label} className="flex flex-col divide-y overflow-hidden rounded-xl border bg-sunken/40">
      {children}
    </ul>
  )
}

export interface RosterRowProps {
  readonly name: string
  /** A small tag after the name ("you"). */
  readonly tag?: string
  /** The line under the name. */
  readonly note?: string
  /** Stands in for the initials in the avatar (an invite's envelope). */
  readonly mark?: string
  /** What is set on it, in the middle (its platform accounts). */
  readonly children?: ReactNode
  /** Its role, at the end. */
  readonly role?: ReactNode
  /** Its actions, last. */
  readonly actions?: ReactNode
}

/** A round avatar in a hue of `name`'s own, with its initials (or `mark`). Decoration only. */
export function Avatar({
  name,
  mark,
  small = false,
}: {
  readonly name: string
  readonly mark?: string
  readonly small?: boolean
}) {
  const hue = hueOf(name)
  return (
    <span
      aria-hidden
      className={classes(
        "inline-flex shrink-0 items-center justify-center rounded-full font-semibold text-text ring-1 ring-white/10",
        small ? "size-8 text-xs" : "size-9 text-[13px]",
      )}
      style={{ background: `linear-gradient(135deg, hsl(${hue} 55% 42% / 0.85), hsl(${(hue + 40) % 360} 45% 28%))` }}
    >
      {mark ?? (initialsOf(name) || "?")}
    </span>
  )
}

export function RosterRow({ name, tag, note, mark, children, role, actions }: RosterRowProps) {
  return (
    <li className="flex flex-wrap items-center gap-x-5 gap-y-3 px-4 py-3.5 transition-colors hover:bg-hover/40">
      <div className="flex min-w-48 flex-1 basis-56 items-center gap-3">
        <Avatar name={name} mark={mark} />
        <div className="flex min-w-0 flex-col">
          <span className="flex items-center gap-2">
            <span className="truncate font-medium font-mono text-sm text-text">{name}</span>
            {tag !== undefined && (
              <span className="rounded-full border px-1.5 py-px text-[10px] text-muted uppercase tracking-wider">
                {tag}
              </span>
            )}
          </span>
          {note !== undefined && <span className="truncate text-muted text-xs">{note}</span>}
        </div>
      </div>
      {children !== undefined && <div className="flex min-w-0 flex-[2] basis-72 flex-wrap gap-1.5">{children}</div>}
      {(role !== undefined || actions !== undefined) && (
        <div className="ms-auto flex shrink-0 items-center gap-3">
          {role}
          {actions}
        </div>
      )}
    </li>
  )
}

export interface RoleSwitchProps<T extends string> {
  readonly label: string
  readonly options: readonly { readonly id: T; readonly label: string; readonly tone: RoleTone }[]
  readonly value: T
  readonly onChange: (id: T) => void
}

export function RoleSwitch<T extends string>({ label, options, value, onChange }: RoleSwitchProps<T>) {
  return (
    <fieldset className="inline-flex gap-0.5 rounded-lg border bg-sunken p-0.5 text-xs">
      <legend className="sr-only">{label}</legend>
      {options.map((o) => (
        <button
          key={o.id}
          type="button"
          aria-pressed={o.id === value}
          onClick={() => o.id !== value && onChange(o.id)}
          className={classes(
            "cursor-pointer rounded-md px-2.5 py-1 font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
            o.id === value ? pressed[o.tone] : "text-muted hover:bg-hover hover:text-text",
          )}
        >
          {o.label}
        </button>
      ))}
    </fieldset>
  )
}

export function RoleBadge({ label, tone }: { readonly label: string; readonly tone: RoleTone }) {
  return <span className={classes("rounded-md px-2 py-0.5 font-medium text-xs", pressed[tone])}>{label}</span>
}
