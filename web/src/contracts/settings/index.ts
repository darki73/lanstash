/**
 * Settings: the settings the web UI may change (GET /api/v1/settings), each with the value in force
 * and where each candidate comes from: the built-in default, the configuration file, a change made
 * here, a command-line flag (which wins, and cannot be changed here). A change applies at the next
 * start: the actions change one, drop a change, and restart lanstash (refused while it is at work,
 * unless forced). noSettings is the actions of a page that cannot change settings.
 */

export interface Setting {
  readonly key: string
  readonly group: string
  readonly label: string
  readonly help: string
  /** Values to pick from (a long list is offered as one types). */
  readonly choices: readonly string[]
  /** Whether the value is several of the choices, comma-separated (absent from older lanstash). */
  readonly multi?: boolean
  /** For a comma-separated value: its parts, each picked from its list (absent from older lanstash). */
  readonly parts?: readonly { readonly label: string; readonly choices: readonly string[] }[]
  /** A choice besides the listed ones that takes a value of its own, and what that looks like. */
  readonly custom?: { readonly label: string; readonly hint: string } | null
  /** The value in force (null: none). */
  readonly value: string | null
  readonly config: string | null
  readonly default: string | null
  /** The change made here, if any. */
  readonly ui: string | null
  readonly flag: boolean
}

/** The settings, or why there are none. */
export type SettingsAnswer = readonly Setting[] | string

export interface SettingsActions {
  read(): Promise<SettingsAnswer>
  /** Changes a setting; the settings again, or why not. */
  set(key: string, value: string): Promise<SettingsAnswer>
  /** Drops the change of a setting; the settings again, or why not. */
  reset(key: string): Promise<SettingsAnswer>
  /** Restarts lanstash; undefined when it is restarting, else why not (such as a prefill running). */
  restart(force?: boolean): Promise<string | undefined>
}

const unavailable = "settings are not available here"

export const noSettings: SettingsActions = {
  read: async () => unavailable,
  set: async () => unavailable,
  reset: async () => unavailable,
  restart: async () => unavailable,
}

/** Where a setting's value in force comes from. */
export type SettingSource = "flag" | "ui" | "config" | "default"

export function source(s: Setting): SettingSource {
  if (s.flag) return "flag"
  if (s.ui !== null) return "ui"
  if (s.config !== null) return "config"
  return "default"
}
