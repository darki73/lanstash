import { describe, expect, it } from "vitest"
import { readBuilds } from "@/adapters/api/builds"
import type { Fetch } from "@/adapters/api/version"

const answering =
  (ok: boolean, body: unknown): Fetch =>
  async () => ({ ok, json: async () => body })

const osi = { game: "osi", product: "osi", version: "3.3", build_config: "ab", current: true, error: null }

describe("adapters/api/builds", () => {
  it("asks the API for the Battle.net builds", async () => {
    const asked: string[] = []
    const fetch: Fetch = async (url) => {
      asked.push(url)
      return { ok: true, json: async () => ({ region: "eu", games: [osi] }) }
    }
    expect(await readBuilds(fetch, "http://box")).toEqual({ region: "eu", games: [osi] })
    expect(asked).toEqual(["http://box/api/v1/builds/blizzard"])
  })
  it("gives undefined for an error, a body that is not builds, or a failed request", async () => {
    expect(await readBuilds(answering(false, { region: "eu", games: [] }), "")).toBeUndefined()
    expect(await readBuilds(answering(true, { region: "eu", games: [{ game: 1 }] }), "")).toBeUndefined()
    expect(await readBuilds(answering(true, null), "")).toBeUndefined()
    expect(
      await readBuilds(async () => {
        throw new Error("offline")
      }, ""),
    ).toBeUndefined()
  })
})
