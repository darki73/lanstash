/**
 * I18n: the words of the app in the reader's language, through i18next (MIT). The only layer that
 * sees i18next: everything above asks for words with useT(namespace) and t("key", values), keys
 * checked against the English catalogue by TypeScript (./catalogue). One instance holds every
 * catalogue lanstash ships (they are bundled, so it is ready at once, in English, which is what a
 * component drawn without a provider gets, as in tests). I18nProvider switches it to a language;
 * useLanguage says which is in use and which way its script runs; useAgo words how long ago a time
 * was, useSpan how long something took (with the plural forms of the language). Values put into words are not
 * escaped here: React escapes what it draws.
 */
import i18next from "i18next"
import { type ReactNode, useEffect } from "react"
import { I18nextProvider, initReactI18next, useTranslation } from "react-i18next"
import { type Catalogue, directionOf, en, fallback, type Language, languages, type Namespace } from "@/i18n/catalogue"
import { since } from "@/lib/ago"
import { span } from "@/lib/span"

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "common"
    resources: Catalogue
  }
}

const words = i18next.createInstance()
void words.use(initReactI18next).init({
  lng: fallback,
  fallbackLng: fallback,
  resources: Object.fromEntries(languages.map((l) => [l.code, l.catalogue])),
  ns: Object.keys(en),
  defaultNS: "common",
  interpolation: { escapeValue: false },
  returnNull: false,
  initAsync: false,
})

/** Draws `children` in `language` (one lanstash ships; else the fallback). */
export function I18nProvider({ language, children }: { readonly language: string; readonly children?: ReactNode }) {
  useEffect(() => {
    if (words.language !== language) void words.changeLanguage(language)
  }, [language])
  return <I18nextProvider i18n={words}>{children}</I18nextProvider>
}

/** The words of `namespace`: `t("key")`, `t("key", { count, name })`. */
export function useT<N extends Namespace>(namespace: N) {
  return useTranslation(namespace).t
}

/** A namespace's `t`, for a helper outside a component that is handed it. */
export type Words<N extends Namespace> = ReturnType<typeof useT<N>>

/** How long ago a time was, in words: `ago(nowMs, thenMs)` ("5 min ago", "never"). */
export function useAgo(): (nowMs: number, thenMs: number) => string {
  const t = useT("common")
  return (nowMs, thenMs) => {
    const s = since(nowMs, thenMs)
    return "count" in s ? t(`ago.${s.unit}`, { count: s.count }) : t(`ago.${s.unit}`)
  }
}

/** A length of time in words: `span(seconds)` ("45 seconds", "1.5 hours"). */
export function useSpan(): (seconds: number) => string {
  const t = useT("common")
  return (seconds) => {
    const s = span(seconds)
    return t(`span.${s.unit}`, { count: s.count })
  }
}

/** The language in use, and which way its script runs. */
export function useLanguage(): { readonly code: string; readonly dir: "ltr" | "rtl" } {
  const { i18n } = useTranslation()
  const code = i18n.resolvedLanguage ?? fallback
  return { code, dir: directionOf(code) }
}

/** The languages a reader may choose. */
export const offered: readonly Pick<Language, "code" | "name">[] = languages.map(({ code, name }) => ({ code, name }))
