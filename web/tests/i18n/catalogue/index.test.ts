import { describe, expect, it } from "vitest"
import { directionOf, en, type Language, languages, pickLanguage } from "@/i18n/catalogue"

const offered: readonly Language[] = [
  { code: "en", name: "English", dir: "ltr", catalogue: en },
  { code: "de", name: "Deutsch", dir: "ltr", catalogue: en },
  { code: "pt-BR", name: "Português", dir: "ltr", catalogue: en },
  { code: "ar", name: "العربية", dir: "rtl", catalogue: en },
]

/** Every key of a catalogue, dotted, with its value. */
function leaves(node: unknown, at = ""): [string, unknown][] {
  if (typeof node !== "object" || node === null) return [[at, node]]
  return Object.entries(node).flatMap(([k, v]) => leaves(v, at === "" ? k : `${at}.${k}`))
}

describe("i18n/catalogue", () => {
  it("picks the saved language, else the browser's first that is offered, else English", () => {
    expect(pickLanguage("de", ["ar"], offered)).toBe("de")
    expect(pickLanguage("xx", ["fr", "de-AT", "ar"], offered)).toBe("de")
    expect(pickLanguage(undefined, ["pt-br"], offered)).toBe("pt-BR")
    expect(pickLanguage(undefined, ["fr", "ja"], offered)).toBe("en")
    expect(pickLanguage(undefined, [])).toBe("en")
  })
  it("knows which way a script runs", () => {
    expect([directionOf("ar", offered), directionOf("de", offered), directionOf("xx", offered)]).toEqual([
      "rtl",
      "ltr",
      "ltr",
    ])
  })
  it("gives every language every key English has, none empty, and the plural forms its own rules need", () => {
    const english = leaves(en).map(([k]) => k)
    const plural = /_(zero|one|two|few|many|other)$/
    const bases = new Set(english.filter((k) => plural.test(k)).map((k) => k.replace(plural, "")))
    for (const l of languages) {
      const own = leaves(l.catalogue)
      const keys = own.map(([k]) => k)
      // Only plural forms of a key English counts may be added (Russian's `_few`, `_many`).
      expect(
        keys.filter((k) => !english.includes(k) && !(plural.test(k) && bases.has(k.replace(plural, "")))),
        l.code,
      ).toEqual([])
      expect(
        english.filter((k) => !keys.includes(k)),
        l.code,
      ).toEqual([])
      // Every form the language's plural rules can pick is there.
      const forms = new Intl.PluralRules(l.code).resolvedOptions().pluralCategories
      const missing = [...bases].flatMap((b) =>
        forms.filter((f) => !keys.includes(`${b}_${f}`)).map((f) => `${b}_${f}`),
      )
      expect(missing, l.code).toEqual([])
      // The same placeholders as English's value (its plural forms: any of English's forms').
      const english_ = new Map(leaves(en))
      const holes = (v: unknown) => new Set(String(v).match(/\{\{\w+\}\}/g) ?? [])
      const wrong = own.filter(([k, v]) => {
        const source = english_.get(k) ?? english_.get(k.replace(plural, "_other"))
        const want = holes(source)
        const have = holes(v)
        return (
          [...have].some((h) => !want.has(h) && h !== "{{count}}") ||
          [...want].some((h) => !have.has(h) && h !== "{{count}}")
        )
      })
      expect(
        wrong.map(([k]) => k),
        l.code,
      ).toEqual([])
      expect(
        own.filter(([, v]) => typeof v !== "string" || v.trim() === "").map(([k]) => k),
        l.code,
      ).toEqual([])
    }
  })
  it("pairs every plural form with its other form", () => {
    const keys = new Set(leaves(en).map(([k]) => k))
    const lonely = [...keys].filter(
      (k) => /_(zero|one|two|few|many)$/.test(k) && !keys.has(k.replace(/_\w+$/, "_other")),
    )
    expect(lonely).toEqual([])
  })
})
