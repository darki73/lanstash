import { describe, expect, it } from "vitest"
import { filled, noStats, type Series, servedGame, total } from "@/contracts/stats"

const MIN = 60_000

const series: Series = {
  step_ms: 2 * MIN,
  names: ["requests.hit_bytes", "downloads.fetched_bytes"],
  buckets: [
    { at_ms: 0, values: [5, 1] },
    { at_ms: 4 * MIN, values: [7, 0] },
  ],
}

describe("contracts/stats", () => {
  it("fills the buckets lanstash left out with zeros", () => {
    expect(filled(series, "requests.hit_bytes", 0, 8 * MIN)).toEqual([5, 0, 7, 0])
    expect(filled(series, "downloads.fetched_bytes", 0, 8 * MIN)).toEqual([1, 0, 0, 0])
    expect(filled(series, "nothing", 0, 8 * MIN)).toEqual([])
  })
  it("sums a counter over the series", () => {
    expect(total(series, "requests.hit_bytes")).toBe(12)
    expect(total(series, "nothing")).toBe(0)
  })
  it("reads the game a served counter is about", () => {
    expect(servedGame("served/steam/1422450")).toEqual({ platform: "steam", game: "1422450" })
    expect(servedGame("served/blizzard/a/b")).toEqual({ platform: "blizzard", game: "a/b" })
    expect(servedGame("served/steam")).toBeUndefined()
    expect(servedGame("requests.hit_bytes")).toBeUndefined()
  })
  it("has a reader for a page with none", async () => {
    expect(await noStats({ names: ["a"], from_ms: 0, to_ms: 1, step_ms: MIN })).toBeUndefined()
  })
})
