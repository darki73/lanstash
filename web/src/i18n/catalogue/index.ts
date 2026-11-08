/**
 * Catalogue: the words of the app in each language lanstash ships, as message catalogues: one
 * JSON file per namespace (a feature, or `common` for the styled pieces) under locales/<code>/.
 * English is the source: every other language holds every key English has (Translation checks it
 * when the catalogue is typed), and the keys `t` takes are English's. A key ending in `_one` or
 * `_other` is a plural form, chosen by the count through the browser's own plural rules.
 * pickLanguage chooses among them: the saved choice, else the browser's first that lanstash has.
 */
import access from "@/i18n/locales/en/access.json"
import clients from "@/i18n/locales/en/clients.json"
import common from "@/i18n/locales/en/common.json"
import dns from "@/i18n/locales/en/dns.json"
import frame from "@/i18n/locales/en/frame.json"
import health from "@/i18n/locales/en/health.json"
import history from "@/i18n/locales/en/history.json"
import palette from "@/i18n/locales/en/palette.json"
import platforms from "@/i18n/locales/en/platforms.json"
import privacy from "@/i18n/locales/en/privacy.json"
import settings from "@/i18n/locales/en/settings.json"
import status from "@/i18n/locales/en/status.json"
import ruAccess from "@/i18n/locales/ru/access.json"
import ruClients from "@/i18n/locales/ru/clients.json"
import ruCommon from "@/i18n/locales/ru/common.json"
import ruDns from "@/i18n/locales/ru/dns.json"
import ruFrame from "@/i18n/locales/ru/frame.json"
import ruHealth from "@/i18n/locales/ru/health.json"
import ruHistory from "@/i18n/locales/ru/history.json"
import ruPalette from "@/i18n/locales/ru/palette.json"
import ruPlatforms from "@/i18n/locales/ru/platforms.json"
import ruPrivacy from "@/i18n/locales/ru/privacy.json"
import ruSettings from "@/i18n/locales/ru/settings.json"
import ruStatus from "@/i18n/locales/ru/status.json"

/** English, the source: its keys are the ones `t` takes. */
export const en = {
  access,
  clients,
  common,
  dns,
  frame,
  health,
  history,
  palette,
  platforms,
  privacy,
  settings,
  status,
}

export type Catalogue = typeof en
export type Namespace = keyof Catalogue

/** A catalogue with every key of `T`, each a string: what another language must hold. */
export type Translation<T = Catalogue> = {
  readonly [K in keyof T]: T[K] extends string ? string : Translation<T[K]>
}

export interface Language {
  /** Its code (BCP 47: "en", "de"). */
  readonly code: string
  /** Its own name for itself ("English", "Deutsch"). */
  readonly name: string
  /** Which way its script runs. */
  readonly dir: "ltr" | "rtl"
  readonly catalogue: Translation
}

/** Russian: every English key, with Russian's further plural forms (`_few`, `_many`). */
const ru: Translation = {
  access: ruAccess,
  clients: ruClients,
  common: ruCommon,
  dns: ruDns,
  frame: ruFrame,
  health: ruHealth,
  history: ruHistory,
  palette: ruPalette,
  platforms: ruPlatforms,
  privacy: ruPrivacy,
  settings: ruSettings,
  status: ruStatus,
}

/** The languages lanstash ships, English first. */
export const languages: readonly Language[] = [
  { code: "en", name: "English", dir: "ltr", catalogue: en },
  { code: "ru", name: "Русский", dir: "ltr", catalogue: ru },
]

/** The language used for what a language lacks, and when none other fits. */
export const fallback = "en"

/**
 * The language to use among `offered`: `saved` when offered, else the first of the browser's
 * `preferred` that is offered, by its whole code ("pt-BR") or its base ("de-AT" as "de"), else
 * the fallback.
 */
export function pickLanguage(
  saved: string | undefined,
  preferred: readonly string[],
  offered: readonly Language[] = languages,
): string {
  const has = (code: string) => offered.find((l) => l.code.toLowerCase() === code.toLowerCase())?.code
  if (saved !== undefined) {
    const kept = has(saved)
    if (kept !== undefined) return kept
  }
  for (const p of preferred) {
    const found = has(p) ?? has(p.split("-")[0] ?? "")
    if (found !== undefined) return found
  }
  return fallback
}

/** Which way `code`'s script runs (left to right for a language lanstash does not ship). */
export const directionOf = (code: string, offered: readonly Language[] = languages): "ltr" | "rtl" =>
  offered.find((l) => l.code === code)?.dir ?? "ltr"
