import { describe, expect, it } from "vitest"
import { readFollowed } from "@/adapters/api/updates"
import type { Fetch } from "@/adapters/api/version"

const answer =
  (ok: boolean, body: unknown, seen: string[] = []): Fetch =>
  async (url) => {
    seen.push(url)
    return { ok, json: async () => body }
  }

describe("adapters/api/updates", () => {
  it("reads the games kept up to date, null when updates are off", async () => {
    const seen: string[] = []
    expect(await readFollowed(answer(true, { followed: ["steam/10", "blizzard/osi"] }, seen), "http://box")).toEqual([
      "steam/10",
      "blizzard/osi",
    ])
    expect(seen).toEqual(["http://box/api/v1/auto-update"])
    expect(await readFollowed(answer(false, { error: "off", kind: "not-found" }), "")).toBeNull()
  })
  it("is undefined for what is not a list", async () => {
    expect(await readFollowed(answer(true, { followed: [1] }), "")).toBeUndefined()
    expect(await readFollowed(answer(false, { error: "boom", kind: "internal" }), "")).toBeUndefined()
    const down: Fetch = async () => {
      throw new Error("offline")
    }
    expect(await readFollowed(down, "")).toBeUndefined()
  })
})
