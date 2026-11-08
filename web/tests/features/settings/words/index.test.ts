import { renderHook } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { Setting } from "@/contracts/settings"
import { settingWords } from "@/features/settings/words"
import { useT } from "@/i18n"

const setting = (key: string): Setting =>
  ({
    key,
    group: "Updates",
    label: "Sent label",
    help: "Sent help",
    choices: [],
    custom: { label: "window", hint: "sent hint" },
    value: null,
  }) as unknown as Setting

describe("features/settings/words", () => {
  it("words a setting it knows from the catalogue, and one it does not as lanstash sent it", () => {
    const { result } = renderHook(() => useT("settings"))
    const w = settingWords(result.current)
    expect([w.label(setting("auto_update")), w.hint(setting("auto_update"))]).toEqual([
      "Automatic updates",
      "HH:MM-HH:MM, UTC",
    ])
    expect([w.label(setting("newer_one")), w.help(setting("newer_one")), w.hint(setting("newer_one"))]).toEqual([
      "Sent label",
      "Sent help",
      "sent hint",
    ])
    expect([w.group("Steam"), w.group("Battle.net"), w.part("Bitness")]).toEqual(["Steam", "Battle.net", "Bitness"])
    expect([w.choice("off"), w.choice("english"), w.choice("constructor")]).toEqual(["off", "english", "constructor"])
  })
})
