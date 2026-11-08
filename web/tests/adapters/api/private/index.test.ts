import { describe, expect, it } from "vitest"
import { privateActions } from "@/adapters/api/private"
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

describe("adapters/api/private", () => {
  it("reads the caller's private games, undefined for anything else", async () => {
    const seen: string[] = []
    const a = privateActions(answer(true, { private: ["steam/10"] }, seen), down, down, "http://box")
    expect(await a.read()).toEqual(["steam/10"])
    expect(seen).toEqual(["http://box/api/v1/private"])
    expect(await privateActions(answer(false, { error: "sign in" }), down, down, "").read()).toBeUndefined()
    expect(await privateActions(answer(true, { private: [1] }), down, down, "").read()).toBeUndefined()
    expect(await privateActions(down, down, down, "").read()).toBeUndefined()
  })
  it("marks with PUT and unmarks with DELETE, giving the list after or why not", async () => {
    const put: string[] = []
    const del: string[] = []
    const a = privateActions(down, answer(true, { private: ["steam/1"] }, put), answer(true, { private: [] }, del), "")
    expect(await a.set("steam", "1", true)).toEqual(["steam/1"])
    expect(await a.set("steam", "1", false)).toEqual([])
    expect([put, del]).toEqual([["/api/v1/private/steam/1"], ["/api/v1/private/steam/1"]])
    const refused = privateActions(down, answer(false, { error: "only a game of one's own" }), down, "")
    expect(await refused.set("steam", "2", true)).toBe("only a game of one's own")
    expect(await privateActions(down, down, down, "").set("steam", "2", true)).toBe("lanstash did not answer")
  })
})
