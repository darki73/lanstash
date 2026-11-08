import { describe, expect, it } from "vitest"
import { type Send, settingsActions } from "@/adapters/api/settings"

const one = {
  key: "steam_os",
  group: "Steam",
  label: "Operating system",
  help: "Who a Steam install is for",
  choices: ["windows", "linux"],
  value: "windows",
  config: null,
  default: "windows",
  ui: null,
  flag: false,
}

describe("adapters/api/settings", () => {
  it("reads, changes, resets and restarts through the API", async () => {
    const sent: string[] = []
    const ok =
      (kind: string, body: unknown): Send =>
      async (url) => {
        sent.push(`${kind} ${url}`)
        return { ok: true, json: async () => body }
      }
    const a = settingsActions(
      ok("GET", { settings: [one] }),
      ok("PUT", { settings: [{ ...one, value: "linux", ui: "linux" }] }),
      ok("DELETE", { settings: [one] }),
      ok("POST", { restarting: true }),
      "http://box",
    )
    expect(await a.read()).toEqual([one])
    expect(await a.set("blizzard_tags", "Windows,x86_64 enUS")).toEqual([{ ...one, value: "linux", ui: "linux" }])
    expect(await a.reset("steam_os")).toEqual([one])
    expect(await a.restart()).toBeUndefined()
    expect(await a.restart(true)).toBeUndefined()
    expect(sent).toEqual([
      "GET http://box/api/v1/settings",
      "PUT http://box/api/v1/settings/blizzard_tags?value=Windows%2Cx86_64%20enUS",
      "DELETE http://box/api/v1/settings/steam_os",
      "POST http://box/api/v1/restart",
      "POST http://box/api/v1/restart?force=1",
    ])
  })
  it("gives the API's reason, an odd answer, or that lanstash did not answer", async () => {
    const refused: Send = async () => ({
      ok: false,
      json: async () => ({ error: "a prefill is running", kind: "busy" }),
    })
    const odd: Send = async () => ({ ok: true, json: async () => ({ settings: [{ key: 1 }] }) })
    const down: Send = async () => {
      throw new Error("offline")
    }
    expect(await settingsActions(refused, refused, refused, refused, "").restart()).toBe("a prefill is running")
    expect(await settingsActions(refused, refused, refused, refused, "").set("a", "b")).toBe("a prefill is running")
    expect(await settingsActions(odd, odd, odd, odd, "").read()).toBe("lanstash refused")
    expect(await settingsActions(down, down, down, down, "").read()).toBe("lanstash did not answer")
    expect(await settingsActions(down, down, down, down, "").restart()).toBe("lanstash did not answer")
  })
})
