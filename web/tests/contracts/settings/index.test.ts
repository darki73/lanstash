import { describe, expect, it } from "vitest"
import { noSettings, type Setting, source } from "@/contracts/settings"

const s = (over: Partial<Setting>): Setting => ({
  key: "max_size",
  group: "Storage",
  label: "Size limit",
  help: "",
  choices: [],
  value: "8000G",
  config: null,
  default: null,
  ui: null,
  flag: false,
  ...over,
})

describe("contracts/settings", () => {
  it("says where a setting's value comes from: a flag, a change here, the config file, the default", () => {
    expect(source(s({ flag: true, ui: "1" }))).toBe("flag")
    expect(source(s({ ui: "6T", config: "8000G" }))).toBe("ui")
    expect(source(s({ config: "8000G" }))).toBe("config")
    expect(source(s({}))).toBe("default")
  })
  it("has no settings where they cannot be changed", async () => {
    expect(await noSettings.read()).toBe("settings are not available here")
    expect(await noSettings.restart()).toBe("settings are not available here")
  })
})
