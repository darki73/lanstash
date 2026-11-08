import { describe, expect, it } from "vitest"
import { type Delete, stashActions } from "@/adapters/api/stash"

describe("adapters/api/stash", () => {
  it("deletes a game by platform and code", async () => {
    const sent: string[] = []
    const del: Delete = async (url) => {
      sent.push(url)
      return { ok: true, json: async () => ({ deleted: "sc1live" }) }
    }
    expect(await stashActions(del, "http://box").remove("blizzard", "sc1live")).toBeUndefined()
    expect(sent).toEqual(["http://box/api/v1/games/blizzard/sc1live"])
  })
  it("gives the API's reason, or that lanstash did not answer", async () => {
    const busy: Delete = async () => ({
      ok: false,
      json: async () => ({ error: "it is being prefilled", kind: "busy" }),
    })
    expect(await stashActions(busy, "").remove("blizzard", "hs")).toBe("it is being prefilled")
    const odd: Delete = async () => ({ ok: false, json: async () => "nope" })
    expect(await stashActions(odd, "").remove("blizzard", "hs")).toBe("lanstash refused")
    const down: Delete = async () => {
      throw new Error("offline")
    }
    expect(await stashActions(down, "").remove("blizzard", "hs")).toBe("lanstash did not answer")
  })
})

describe("adapters/api/stash prune", () => {
  it("asks what a prune would delete and starts one", async () => {
    const sent: string[] = []
    const ok = (body: unknown) => async (url: string) => {
      sent.push(url)
      return { ok: true, json: async () => body }
    }
    const a = stashActions(ok({}), "http://box", ok({ pruning: "wow" }), ok({ files: 3, bytes: 9, recent_bytes: 1 }))
    expect(await a.prunable?.("blizzard", "wow")).toEqual({ files: 3, bytes: 9, recent_bytes: 1 })
    expect(await a.prune?.("blizzard", "wow")).toBeUndefined()
    expect(sent).toEqual(["http://box/api/v1/games/blizzard/wow/prune", "http://box/api/v1/games/blizzard/wow/prune"])
    expect(stashActions(ok({}), "").prunable, "no prune without POST and GET").toBeUndefined()
    const steam = stashActions(ok({}), "", ok({}), ok({ files: 3, compact: 2, bytes: 9, recent_bytes: 0 }))
    expect(await steam.prunable?.("steam", "100"), "compacted segments").toEqual({
      files: 3,
      bytes: 9,
      recent_bytes: 0,
      compact: 2,
    })
    const odd = stashActions(ok({}), "", ok({}), ok({ files: "x" }))
    expect(await odd.prunable?.("blizzard", "wow")).toBe("lanstash refused")
  })
})
