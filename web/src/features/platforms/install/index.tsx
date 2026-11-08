/**
 * Install: on a game's page, a prefill for another install than lanstash is set up for (a Mac in a
 * LAN of Windows PCs, a German client among English ones): the platforms the game has installs
 * for, an architecture when it has several (any: every architecture of that platform), a language
 * when it has several (as set up: the languages lanstash is set up for), and the prefill, queued
 * like any other. It fills what that install takes and the stash lacks; the usual install is not
 * changed.
 */
import { useState } from "react"
import type { GameInstalls } from "@/contracts/game"
import { useLanguage, useT } from "@/i18n"
import { Button } from "@/styled/button"
import { Field, Select } from "@/styled/field"
import { Panel } from "@/styled/panel"
import { Segmented } from "@/styled/segmented"

/** The choice that leaves the architecture or the language as it is. */
const ANY = "any"

/** How people know a platform: Battle.net's manifest says `OSX`, Steam says `windows`, `macos`, `linux`. */
const NAMES: Readonly<Record<string, string>> = { OSX: "macOS", macos: "macOS", windows: "Windows", linux: "Linux" }

/** A platform's tag as people know it. */
export const named = (tag: string) => NAMES[tag] ?? tag

/** Whether a tag is a Battle.net locale (`deDE`). */
const isLocale = (tag: string) => /^[a-z]{2}[A-Z]{2}$/.test(tag)

/** A locale tag as people read it in `language` (`deDE`: "German (Germany)"), the tag when unknown. */
export function localeName(tag: string, language: string): string {
  try {
    return new Intl.DisplayNames([language], { type: "language" }).of(`${tag.slice(0, 2)}-${tag.slice(2)}`) ?? tag
  } catch {
    return tag
  }
}

/**
 * An install's tags as people read them in `language`: `OSX,arm64` is "macOS arm64", Steam's
 * `linux` "Linux", `OSX,deDE` "macOS, German (Germany)".
 */
export function installName(install: string, language: string): string {
  const tags = install.split(",").filter((x) => x !== "")
  const machine = tags.filter((x) => !isLocale(x)).map(named)
  const spoken = tags.filter(isLocale).map((l) => localeName(l, language))
  return [...(machine.length > 0 ? [machine.join(" ")] : []), ...spoken].join(", ")
}

/** Whether the stash holds all of `platform`'s install, as far as `installs` says. */
export function heldWhole(installs: GameInstalls, platform: string): boolean {
  const h = installs.held?.[platform]
  return h != null && h.want > 0 && h.have >= h.want
}

export interface InstallProps {
  /** What the game has installs for (Battle.net: the download manifest's tags; Steam: the systems). */
  readonly installs: GameInstalls
  /** The game, as its buttons name it. */
  readonly name: string
  /** Prefills the install named by its tags (`OSX`, `OSX,arm64`, `deDE`). */
  readonly start: (install: string) => void
}

export function Install({ installs, name, start }: InstallProps) {
  const t = useT("platforms")
  const { code } = useLanguage()
  // At first the platform most likely wanted: another than the one set up, not stashed whole yet.
  const [platform, setPlatform] = useState(
    installs.platforms.find((p) => p !== installs.own && !heldWhole(installs, p)) ?? installs.platforms[0] ?? "",
  )
  const [architecture, setArchitecture] = useState(ANY)
  const [locale, setLocale] = useState(ANY)
  const locales = installs.locales ?? []
  const platforms = installs.platforms.length > 1
  // One platform and one language (Windows only, English only): nothing else to download for.
  if (!platforms && locales.length < 2) return null
  const install = [
    platforms ? platform : undefined,
    platforms && architecture !== ANY ? architecture : undefined,
    locale === ANY ? undefined : locale,
  ].filter((x) => x !== undefined)
  // The button names the platform and the language, the architecture only in the job's title.
  const what = install.length === 0 ? undefined : installName(install.filter((x) => x !== architecture).join(","), code)
  // That platform's install as set up otherwise, held whole already: nothing to prefill.
  const stashed = platforms && locale === ANY && architecture === ANY && heldWhole(installs, platform)
  return (
    <Panel title={t("install.title")}>
      {platforms && (
        <Field label={t("install.platform")} help={t("install.help")}>
          <Segmented
            label={t("install.platform")}
            options={installs.platforms.map((p) => ({
              id: p,
              label: heldWhole(installs, p) ? t("install.held", { platform: named(p) }) : named(p),
            }))}
            value={platform}
            onChange={setPlatform}
          />
        </Field>
      )}
      {platforms && installs.architectures.length > 1 && (
        <Field label={t("install.architecture")} help={t("install.architectureHelp")}>
          <Select
            label={t("install.architecture")}
            value={architecture}
            options={[ANY, ...installs.architectures]}
            names={{ [ANY]: t("install.any") }}
            onChange={setArchitecture}
          />
        </Field>
      )}
      {locales.length > 1 && (
        <Field label={t("install.language")} help={t("install.languageHelp")}>
          <Select
            label={t("install.language")}
            value={locale}
            options={[ANY, ...locales]}
            names={{ [ANY]: t("install.asSetUp"), ...Object.fromEntries(locales.map((l) => [l, localeName(l, code)])) }}
            onChange={setLocale}
          />
        </Field>
      )}
      <Field label={t("install.prefill")} help={t("install.prefillHelp")}>
        {stashed ? (
          <Button disabled onClick={() => {}}>
            {t("install.stashed", { platform: named(platform) })}
          </Button>
        ) : (
          <Button
            tone="accent"
            disabled={what === undefined}
            onClick={() => start(install.join(","))}
            label={what === undefined ? t("install.choose") : t("install.startLabel", { name, platform: what })}
          >
            {what === undefined ? t("install.choose") : t("install.start", { platform: what })}
          </Button>
        )}
      </Field>
    </Panel>
  )
}
