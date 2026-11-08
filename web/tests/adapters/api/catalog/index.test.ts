import { describe, expect, it } from "vitest"
import { readCatalog } from "@/adapters/api/catalog"
import type { Fetch } from "@/adapters/api/version"

const s1 = { code: "s1", name: "StarCraft", game: "sc1live", version: "1.23", known: true }

describe("adapters/api/catalog", () => {
  it("asks the API for the Battle.net catalog", async () => {
    const asked: string[] = []
    const fetch: Fetch = async (url) => {
      asked.push(url)
      return { ok: true, json: async () => ({ products: [s1] }) }
    }
    expect(await readCatalog(fetch, "http://box")).toEqual({ products: [s1] })
    expect(asked).toEqual(["http://box/api/v1/catalog/blizzard"])
    const game = { code: "7", name: "G", game: "7", version: "9", known: true, download: 100 }
    const steam: Fetch = async (url) => {
      asked.push(url)
      return { ok: true, json: async () => ({ products: [game] }) }
    }
    expect(await readCatalog(steam, "", "steam")).toEqual({ products: [game] })
    expect(asked.at(-1)).toBe("/api/v1/catalog/steam")
  })
  it("gives undefined for an error, a body that is not a catalog, or a failed request", async () => {
    const answering =
      (ok: boolean, body: unknown): Fetch =>
      async () => ({ ok, json: async () => body })
    expect(await readCatalog(answering(false, { products: [] }), "")).toBeUndefined()
    expect(await readCatalog(answering(true, { products: [{ code: 1 }] }), "")).toBeUndefined()
    expect(
      await readCatalog(async () => {
        throw new Error("offline")
      }, ""),
    ).toBeUndefined()
  })
})
