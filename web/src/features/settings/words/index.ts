/**
 * Words: what the settings page says of each setting, in the reader's language: its label and help,
 * its group, the parts of a value in parts, the choices that are words (always, off, weekly, ...;
 * not values such as a language or a size) and what a value of its own looks like. lanstash sends
 * them in English with the settings; a setting or a word the catalogue does not know yet (one a
 * newer lanstash added) is shown as lanstash sent it.
 */
import type { Setting } from "@/contracts/settings"
import type { Words } from "@/i18n"
import { en } from "@/i18n/catalogue"

const words = en.settings

/** Whether `key` is one of `known`'s keys. */
const knows = <T extends object>(known: T, key: string): key is Extract<keyof T, string> => Object.hasOwn(known, key)

export interface SettingWords {
  readonly label: (s: Setting) => string
  readonly help: (s: Setting) => string
  readonly group: (group: string) => string
  readonly part: (part: string) => string
  /** A choice in words (a value that is no word comes back as it is). */
  readonly choice: (value: string) => string
  /** What a value of the setting's own looks like. */
  readonly hint: (s: Setting) => string | undefined
}

export function settingWords(t: Words<"settings">): SettingWords {
  return {
    label: (s) => (knows(words.setting, s.key) ? t(`setting.${s.key}.label`) : s.label),
    help: (s) => (knows(words.setting, s.key) ? t(`setting.${s.key}.help`) : s.help),
    group: (g) => (knows(words.group, g) ? t(`group.${g}`) : g),
    part: (p) => (knows(words.part, p) ? t(`part.${p}`) : p),
    choice: (c) => (knows(words.choice, c) ? t(`choice.${c}`) : c),
    hint: (s) => (knows(words.hint, s.key) ? t(`hint.${s.key}`) : s.custom?.hint),
  }
}
