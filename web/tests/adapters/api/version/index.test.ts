import { describe, expect, it } from "vitest"
import { type Fetch, readVersion } from "@/adapters/api/version"

const answering =
  (ok: boolean, body: unknown): Fetch =>
  async () => ({ ok, json: async () => body })

describe("adapters/api/version", () => {
  it("asks the API which version runs", async () => {
    const asked: string[] = []
    const fetch: Fetch = async (url) => {
      asked.push(url)
      return { ok: true, json: async () => ({ version: "0.1.0" }) }
    }
    expect(await readVersion(fetch, "http://box")).toEqual({ version: "0.1.0" })
    expect(asked).toEqual(["http://box/api/v1/version"])
  })
  it("gives undefined for an error, a body that is not a version, or a failed request", async () => {
    expect(await readVersion(answering(false, { version: "0.1.0" }), "")).toBeUndefined()
    expect(await readVersion(answering(true, { other: 1 }), "")).toBeUndefined()
    expect(await readVersion(answering(true, null), "")).toBeUndefined()
    expect(
      await readVersion(async () => {
        throw new Error("offline")
      }, ""),
    ).toBeUndefined()
  })
})
