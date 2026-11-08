import { describe, expect, it } from "vitest"
import { readGame } from "@/adapters/api/game"
import type { Fetch } from "@/adapters/api/version"

const osi = { game: "osi", product: "osi", region: "eu", build: null, prerelease: null, error: null }

describe("adapters/api/game", () => {
  it("asks the API about one game", async () => {
    const asked: string[] = []
    const fetch: Fetch = async (url) => {
      asked.push(url)
      return { ok: true, json: async () => osi }
    }
    expect(await readGame(fetch, "http://box", "Hero-Live-a")).toEqual(osi)
    expect(asked).toEqual(["http://box/api/v1/games/blizzard/Hero-Live-a"])
  })
  it("gives undefined for an error, a body that is not a game, or a failed request", async () => {
    const answering =
      (ok: boolean, body: unknown): Fetch =>
      async () => ({ ok, json: async () => body })
    expect(await readGame(answering(false, osi), "", "osi")).toBeUndefined()
    expect(await readGame(answering(true, { game: "osi" }), "", "osi")).toBeUndefined()
    expect(
      await readGame(
        async () => {
          throw new Error("offline")
        },
        "",
        "osi",
      ),
    ).toBeUndefined()
  })
})
