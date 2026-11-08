import { describe, expect, it } from "vitest"
import { since } from "@/lib/ago"

const now = 1_000_000_000_000
const earlier = (s: number) => since(now, now - s * 1000)

describe("lib/ago", () => {
  it("measures how long ago, coarser the longer it is", () => {
    expect([
      earlier(5),
      earlier(5 * 60),
      earlier(3 * 3600),
      earlier(47 * 3600),
      earlier(3 * 86400),
      earlier(30 * 86400),
    ]).toEqual([
      { unit: "now" },
      { unit: "minutes", count: 5 },
      { unit: "hours", count: 3 },
      { unit: "hours", count: 47 },
      { unit: "days", count: 3 },
      { unit: "weeks", count: 4 },
    ])
  })
  it("is never for time 0, and now for a time ahead", () => {
    expect([since(now, 0), since(now, now + 5000)]).toEqual([{ unit: "never" }, { unit: "now" }])
  })
})
