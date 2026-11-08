import { describe, expect, it } from "vitest"
import { healthReader } from "@/adapters/api/health"
import type { Fetch } from "@/adapters/api/version"

const answering =
  (ok: boolean, body: unknown): Fetch =>
  async () => ({ ok, json: async () => body })

const check = { level: "warn", what: "open files", detail: "lanstash: 900 of 1024 (88 %)" }

describe("adapters/api/health", () => {
  it("asks the API for the service's checks, leaving out what is not one", async () => {
    const asked: string[] = []
    const fetch: Fetch = async (url) => {
      asked.push(url)
      return { ok: true, json: async () => ({ checks: [check, { ...check, level: "odd" }, { what: 1 }] }) }
    }
    expect(await healthReader(fetch, "http://box")()).toEqual([check])
    expect(asked).toEqual(["http://box/api/v1/health"])
  })
  it("gives undefined for an error, a body that is not checks, or a failed request", async () => {
    expect(await healthReader(answering(false, { checks: [] }), "")()).toBeUndefined()
    expect(await healthReader(answering(true, { checks: 1 }), "")()).toBeUndefined()
    expect(
      await healthReader(async () => {
        throw new Error("offline")
      }, "")(),
    ).toBeUndefined()
  })
})
