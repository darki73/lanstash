import { describe, expect, it } from "vitest"
import { hidingActions, readHidden } from "@/adapters/api/hidden"
import type { Fetch } from "@/adapters/api/version"

const answer =
  (ok: boolean, body: unknown, seen: string[] = []): Fetch =>
  async (url) => {
    seen.push(url)
    return { ok, json: async () => body }
  }

const down: Fetch = async () => {
  throw new Error("offline")
}

describe("adapters/api/hidden", () => {
  it("reads the hidden games, null from a lanstash that keeps none", async () => {
    const seen: string[] = []
    expect(await readHidden(answer(true, { hidden: ["steam/10"] }, seen), "http://box")).toEqual(["steam/10"])
    expect(seen).toEqual(["http://box/api/v1/hidden"])
    expect(await readHidden(answer(false, { error: "x", kind: "not-found" }), "")).toBeNull()
    expect(await readHidden(answer(true, { hidden: [1] }), "")).toBeUndefined()
    expect(await readHidden(down, "")).toBeUndefined()
  })
  it("hides with PUT and shows with DELETE, giving the list after or why not", async () => {
    const put: string[] = []
    const del: string[] = []
    const a = hidingActions(answer(true, { hidden: ["steam/1"] }, put), answer(true, { hidden: [] }, del), "")
    expect(await a.set("steam", "1", true)).toEqual(["steam/1"])
    expect(await a.set("steam", "1", false)).toEqual([])
    expect([put, del]).toEqual([["/api/v1/hidden/steam/1"], ["/api/v1/hidden/steam/1"]])
    const refused = hidingActions(answer(false, { error: "not a game" }), down, "")
    expect(await refused.set("steam", "x", true)).toBe("not a game")
    expect(await refused.set("steam", "x", false)).toBe("lanstash did not answer")
  })
})
