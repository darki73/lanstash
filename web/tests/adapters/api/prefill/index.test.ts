import { describe, expect, it } from "vitest"
import { type Post, prefillActions } from "@/adapters/api/prefill"

describe("adapters/api/prefill", () => {
  it("posts a start and a stop", async () => {
    const posted: string[] = []
    const post: Post = async (url) => {
      posted.push(url)
      return { ok: true, json: async () => ({}) }
    }
    const a = prefillActions(post, "http://box")
    expect(await a.start("blizzard", "Hero-Live-a")).toBeUndefined()
    expect(await a.start("blizzard", "hs", true)).toBeUndefined()
    expect(await a.start("blizzard", "ovw", false, true)).toBeUndefined()
    expect(await a.start("blizzard", "ovw", true, true)).toBeUndefined()
    expect(await a.start("steam", "427520", false, true, true)).toBeUndefined()
    expect(await a.start("steam", "570", false, "only", true)).toBeUndefined()
    expect(await a.start("blizzard", "fenris", false, false, true, "OSX,arm64")).toBeUndefined()
    await a.stop()
    await a.stop("steam")
    expect(posted).toEqual([
      "http://box/api/v1/prefill/blizzard/Hero-Live-a",
      "http://box/api/v1/prefill/blizzard/hs?build=pre",
      "http://box/api/v1/prefill/blizzard/ovw?from=lancache",
      "http://box/api/v1/prefill/blizzard/ovw?build=pre&from=lancache",
      "http://box/api/v1/prefill/steam/427520?from=lancache&queue=1",
      "http://box/api/v1/prefill/steam/570?from=lancache-only&queue=1",
      "http://box/api/v1/prefill/blizzard/fenris?queue=1&install=OSX%2Carm64",
      "http://box/api/v1/prefill/stop",
      "http://box/api/v1/prefill/stop?platform=steam",
    ])
  })
  it("takes a game out of a queue with a DELETE, and says why it could not", async () => {
    const deleted: string[] = []
    const del: Post = async (url) => {
      deleted.push(url)
      return url.endsWith("/7")
        ? { ok: false, json: async () => ({ error: "that game is not queued", kind: "not-found" }) }
        : { ok: true, json: async () => ({}) }
    }
    const post: Post = async () => ({ ok: true, json: async () => ({}) })
    const a = prefillActions(post, "http://box", del)
    expect(await a.unqueue("steam", "570")).toBeUndefined()
    expect(await a.unqueue("steam", "7")).toBe("that game is not queued")
    expect(deleted).toEqual(["http://box/api/v1/prefill/steam/570", "http://box/api/v1/prefill/steam/7"])
    expect(await prefillActions(post, "").unqueue("steam", "570")).toBe(
      "lanstash cannot take games out of the queue here",
    )
  })
  it("gives the API's reason, or that lanstash did not answer", async () => {
    const busy: Post = async () => ({ ok: false, json: async () => ({ error: "a prefill is running", kind: "busy" }) })
    expect(await prefillActions(busy, "").start("blizzard", "hs")).toBe("a prefill is running")
    const odd: Post = async () => ({ ok: false, json: async () => "nope" })
    expect(await prefillActions(odd, "").start("blizzard", "hs")).toBe("lanstash refused")
    const down: Post = async () => {
      throw new Error("offline")
    }
    expect(await prefillActions(down, "").start("blizzard", "hs")).toBe("lanstash did not answer")
    await prefillActions(down, "").stop()
  })
})
