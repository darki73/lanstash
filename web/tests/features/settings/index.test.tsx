import { fireEvent, render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { Setting, SettingsActions } from "@/contracts/settings"
import { SettingsView } from "@/features/settings"

const base = (over: Partial<Setting>): Setting => ({
  key: "max_size",
  group: "Storage",
  label: "Size limit",
  help: "The most the stash may hold",
  choices: [],
  value: "8000G",
  config: "8000G",
  default: null,
  ui: null,
  flag: false,
  ...over,
})

function fake(start: readonly Setting[], restartAnswers: (string | undefined)[] = [undefined]) {
  let list = [...start]
  const calls: string[] = []
  const actions: SettingsActions = {
    read: async () => list,
    set: async (key, value) => {
      calls.push(`set ${key}=${value}`)
      if (value === "bad") return "`max_size`: invalid value"
      list = list.map((s) => (s.key === key ? { ...s, value, ui: value } : s))
      return list
    },
    reset: async (key) => {
      calls.push(`reset ${key}`)
      list = list.map((s) => (s.key === key ? { ...s, value: s.config, ui: null } : s))
      return list
    },
    restart: async (force) => {
      calls.push(`restart${force ? " forced" : ""}`)
      return restartAnswers.shift()
    },
  }
  return { actions, calls }
}

describe("features/settings", () => {
  it("groups the settings, says where each value comes from, and changes one typed in", async () => {
    const { actions, calls } = fake([
      base({}),
      base({
        key: "steam_os",
        group: "Steam",
        label: "Operating system",
        choices: ["windows", "linux"],
        value: "windows",
        config: null,
        default: "windows",
      }),
      base({ key: "prefill_parallel", group: "Updates", label: "Prefill requests at once", value: "4", flag: true }),
    ])
    render(<SettingsView actions={actions} />)
    expect(await screen.findByRole("region", { name: "Storage" })).toBeTruthy()
    expect(screen.getByRole("region", { name: "Steam" })).toBeTruthy()
    expect(screen.getByText("config file")).toBeTruthy()
    expect(screen.getByText("default")).toBeTruthy()
    expect(screen.getByText("command-line flag")).toBeTruthy()
    expect(screen.queryByRole("textbox", { name: "Prefill requests at once value" }), "a flag's").toBeNull()
    const box = screen.getByRole("textbox", { name: "Size limit value" })
    fireEvent.change(box, { target: { value: "6T" } })
    fireEvent.click(screen.getByRole("button", { name: "Save Size limit" }))
    expect(await screen.findByText("changed here")).toBeTruthy()
    fireEvent.click(
      within(screen.getByRole("group", { name: "Operating system" })).getByRole("button", { name: "linux" }),
    )
    await screen.findAllByText("changed here")
    expect(calls).toEqual(["set max_size=6T", "set steam_os=linux"])
    fireEvent.click(screen.getByRole("button", { name: "Reset Size limit to the config file's value" }))
    await screen.findByText("config file")
    expect(calls.at(-1)).toBe("reset max_size")
  })
  it("offers a window of its own, Battle.net's tags in parts, and Steam's languages as one types", async () => {
    const { actions, calls } = fake([
      base({
        key: "auto_update",
        group: "Updates",
        label: "Automatic updates",
        choices: ["always", "off"],
        custom: { label: "window", hint: "HH:MM-HH:MM, UTC" },
        value: "always",
      }),
      base({
        key: "blizzard_tags",
        group: "Battle.net",
        label: "Install tags",
        value: "Windows,x86_64,enUS,HD",
        parts: [
          { label: "Platform", choices: ["Windows", "OSX"] },
          { label: "Bitness", choices: ["x86_64", "arm64"] },
          { label: "Language", choices: ["enUS", "deDE"] },
        ],
      }),
      base({
        key: "steam_language",
        group: "Steam",
        label: "Languages",
        choices: ["english", "german", "french", "italian", "spanish", "russian", "polish"],
        value: "english",
      }),
    ])
    render(<SettingsView actions={actions} />)
    const updates = within(await screen.findByRole("group", { name: "Automatic updates" }))
    expect(screen.queryByRole("textbox", { name: "Automatic updates value" }), "no box for always").toBeNull()
    fireEvent.click(updates.getByRole("button", { name: "window" }))
    const box = screen.getByRole("textbox", { name: "Automatic updates value" })
    expect(box.getAttribute("placeholder")).toBe("HH:MM-HH:MM, UTC")
    fireEvent.change(screen.getByRole("combobox", { name: "Install tags: Language" }), { target: { value: "deDE" } })
    expect(calls.at(-1)).toBe("set blizzard_tags=Windows,x86_64,deDE,HD")
    expect(screen.getByText("+ HD"), "unknown tags kept").toBeTruthy()
    fireEvent.change(screen.getByRole("combobox", { name: "Languages" }), { target: { value: "german" } })
    expect(calls.at(-1), "a long list is a dropdown").toBe("set steam_language=german")
    expect(screen.queryByRole("group", { name: "Languages" }), "no row of seven choices").toBeNull()
  })
  it("picks several of Steam's languages", async () => {
    const { actions, calls } = fake([
      base({
        key: "steam_language",
        group: "Steam",
        label: "Languages",
        choices: ["english", "german", "french", "italian", "spanish", "russian", "polish"],
        multi: true,
        value: "english,german",
      }),
    ])
    render(<SettingsView actions={actions} />)
    fireEvent.change(await screen.findByRole("combobox", { name: "Add to Languages" }), { target: { value: "polish" } })
    expect(calls.at(-1)).toBe("set steam_language=english,german,polish")
    fireEvent.click(await screen.findByRole("button", { name: "Drop english from Languages" }))
    expect(calls.at(-1)).toBe("set steam_language=german,polish")
    expect(screen.queryByRole("textbox"), "no box to type in").toBeNull()
  })
  it("shows why a change was refused", async () => {
    const { actions } = fake([base({})])
    render(<SettingsView actions={actions} />)
    const box = await screen.findByRole("textbox", { name: "Size limit value" })
    fireEvent.change(box, { target: { value: "bad" } })
    fireEvent.keyDown(box, { key: "Enter" })
    expect(await screen.findByText("`max_size`: invalid value")).toBeTruthy()
  })
  it("offers a restart once something changed, and Restart anyway while lanstash is at work", async () => {
    const { actions, calls } = fake([base({})], ["a prefill is running", undefined])
    const { rerender } = render(<SettingsView actions={actions} started={1000} />)
    const box = await screen.findByRole("textbox", { name: "Size limit value" })
    expect(screen.queryByRole("region", { name: "Apply the changes" })).toBeNull()
    fireEvent.change(box, { target: { value: "6T" } })
    fireEvent.keyDown(box, { key: "Enter" })
    fireEvent.click(await screen.findByRole("button", { name: "Restart lanstash to apply the changes" }))
    expect(await screen.findByText("Not now: a prefill is running.")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Restart anyway" }))
    expect(await screen.findByText(/lanstash is restarting/)).toBeTruthy()
    expect(calls.slice(-2)).toEqual(["restart", "restart forced"])
    const again = screen.getByRole("button", { name: "lanstash is restarting" }) as HTMLButtonElement
    expect(again.disabled, "no second restart while it restarts").toBe(true)
    // The same lanstash still answering: still restarting.
    rerender(<SettingsView actions={actions} started={1000} />)
    expect(screen.getByText(/lanstash is restarting/)).toBeTruthy()
    // Back, started anew: the bar goes, and the settings show as the new lanstash has them.
    rerender(<SettingsView actions={actions} started={2000} />)
    expect(await screen.findByRole("textbox", { name: "Size limit value" })).toBeTruthy()
    expect(screen.queryByText(/lanstash is restarting/)).toBeNull()
    expect(screen.queryByRole("region", { name: "Apply the changes" })).toBeNull()
  })
  it("says when there are no settings to change", async () => {
    render(<SettingsView />)
    expect(await screen.findByText("settings are not available here")).toBeTruthy()
  })
})
