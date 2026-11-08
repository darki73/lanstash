/**
 * Roles: how the accounts pages show a role (its tone; its name and what it may do are the access
 * words `role.<id>` and `roleWords.<id>`) and a platform account (its platform faint before its
 * name).
 */
import type { User } from "@/contracts/session"
import type { Words } from "@/i18n"
import type { RoleTone } from "@/styled/roster"
import type { ToggleOption } from "@/styled/toggles"

export type Role = User["role"]

/** The roles an account can have, least first, with their tones. */
export const roles: readonly { readonly id: Role; readonly tone: RoleTone }[] = [
  { id: "viewer", tone: "least" },
  { id: "player", tone: "middle" },
  { id: "admin", tone: "strong" },
]

/** The roles as a switch shows them, named in the reader's language. */
export const roleOptions = (t: Words<"access">) => roles.map((r) => ({ ...r, label: t(`role.${r.id}`) }))

/** A role's tone. */
export const toneOf = (role: Role): RoleTone => roles.find((r) => r.id === role)?.tone ?? "least"

/** A platform account (`steam/someone`) as a toggle: its platform first, faint. */
export const toggleOf = (link: string): ToggleOption => {
  const [group, ...rest] = link.split("/")
  return rest.length === 0 ? { id: link, label: link } : { id: link, group, label: rest.join("/") }
}

/** The platform accounts to choose from: those lanstash is signed in to, then any `kept` beside them. */
export const linkOptions = (choices: readonly string[], kept: readonly string[] = []): readonly ToggleOption[] =>
  [...choices, ...kept.filter((k) => !choices.includes(k))].map(toggleOf)
