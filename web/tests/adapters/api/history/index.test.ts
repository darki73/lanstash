import { describe, expect, it } from "vitest"
import { readHistory } from "@/adapters/api/history"
import type { Fetch } from "@/adapters/api/version"

const answering =
  (ok: boolean, body: unknown): Fetch =>
  async () => ({ ok, json: async () => body })

const event = { at_ms: 5, platform: "steam", game: "7", change: "new-build", version: "2", from: "1" }

describe("adapters/api/history", () => {
  it("asks the API what was updated when", async () => {
    const asked: string[] = []
    const fetch: Fetch = async (url) => {
      asked.push(url)
      return { ok: true, json: async () => ({ events: [event] }) }
    }
    expect(await readHistory(fetch, "http://box")).toEqual({ events: [event] })
    expect(asked).toEqual(["http://box/api/v1/history"])
  })
  it("leaves out what is not an event", async () => {
    const odd = [{ ...event, change: "gone" }, { ...event, at_ms: "5" }, { ...event, from: 1 }, null]
    expect(await readHistory(answering(true, { events: [event, ...odd] }), "")).toEqual({ events: [event] })
  })
  it("gives undefined for an error, a body that is not a history, or a failed request", async () => {
    expect(await readHistory(answering(false, { events: [] }), "")).toBeUndefined()
    expect(await readHistory(answering(true, { events: 1 }), "")).toBeUndefined()
    expect(await readHistory(answering(true, null), "")).toBeUndefined()
    expect(
      await readHistory(async () => {
        throw new Error("offline")
      }, ""),
    ).toBeUndefined()
  })
})
