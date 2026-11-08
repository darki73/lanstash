import { describe, expect, it } from "vitest"
import { statsReader, topReader } from "@/adapters/api/stats"
import type { Fetch } from "@/adapters/api/version"

const answering =
  (ok: boolean, body: unknown): Fetch =>
  async () => ({ ok, json: async () => body })

const series = { step_ms: 60_000, names: ["requests.hit_bytes"], buckets: [{ at_ms: 0, values: [5] }] }

describe("adapters/api/stats", () => {
  it("asks the API for the counters over time", async () => {
    const asked: string[] = []
    const fetch: Fetch = async (url) => {
      asked.push(url)
      return { ok: true, json: async () => series }
    }
    const got = await statsReader(
      fetch,
      "http://box",
    )({
      names: ["requests.hit_bytes", "downloads.fetched_bytes"],
      from_ms: 0,
      to_ms: 3_600_000.7,
      step_ms: 60_000,
    })
    expect(got).toEqual(series)
    expect(asked).toEqual([
      "http://box/api/v1/stats?names=requests.hit_bytes,downloads.fetched_bytes&from=0&to=3600000&step=60000",
    ])
  })
  it("asks for the counters that went up most, leaving out what is not one", async () => {
    const asked: string[] = []
    const fetch: Fetch = async (url) => {
      asked.push(url)
      return { ok: true, json: async () => ({ top: [{ name: "served/steam/7", value: 5 }, { name: 1 }] }) }
    }
    expect(await topReader(fetch, "http://box")("served/", 0, 60_000, 8)).toEqual([
      { name: "served/steam/7", value: 5 },
    ])
    expect(asked).toEqual(["http://box/api/v1/stats/top?prefix=served%2F&from=0&to=60000&limit=8"])
    expect(await topReader(answering(false, {}), "")("served/", 0, 1, 1)).toBeUndefined()
  })
  it("gives undefined for an error, a body that is not a series, or a failed request", async () => {
    const q = { names: ["a"], from_ms: 0, to_ms: 1, step_ms: 60_000 }
    expect(await statsReader(answering(false, series), "")(q)).toBeUndefined()
    expect(
      await statsReader(answering(true, { ...series, buckets: [{ at_ms: 0, values: ["5"] }] }), "")(q),
    ).toBeUndefined()
    expect(await statsReader(answering(true, null), "")(q)).toBeUndefined()
    expect(
      await statsReader(async () => {
        throw new Error("offline")
      }, "")(q),
    ).toBeUndefined()
  })
})
