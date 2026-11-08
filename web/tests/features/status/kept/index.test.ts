import { describe, expect, it } from "vitest"
import { kept, merged, minuteOf } from "@/features/status/kept"
import { ranges } from "@/features/status/range"

const step = ranges.day.step
const to = 1_000 * 3_600_000

describe("features/status/kept", () => {
  it("reads a series as values per bucket, totals and the last bucket that rose", () => {
    const k = kept(
      {
        step_ms: step,
        names: ["a", "b"],
        buckets: [
          { at_ms: to - 3 * step, values: [1, 0] },
          { at_ms: to - step, values: [0, 4] },
        ],
      },
      "day",
      to,
    )
    expect(k.values("a").slice(-3)).toEqual([1, 0, 0])
    expect(k.values("b").slice(-1)).toEqual([4])
    expect(k.values("a")).toHaveLength(ranges.day.span / step)
    expect([k.total("a"), k.total("b"), k.total("c")]).toEqual([1, 4, 0])
    expect(k.lastRise(["a"])).toBe(to - 2 * step)
    expect(k.lastRise(["b"])).toBe(to)
    expect(k.lastRise(["c"])).toBe(0)
  })
  it("puts series of the same buckets side by side", () => {
    const m = merged([
      { step_ms: step, names: ["a"], buckets: [{ at_ms: 10, values: [1] }] },
      { step_ms: step, names: ["b", "c"], buckets: [{ at_ms: 20, values: [2, 3] }] },
    ])
    expect(m).toEqual({
      step_ms: step,
      names: ["a", "b", "c"],
      buckets: [
        { at_ms: 10, values: [1, 0, 0] },
        { at_ms: 20, values: [0, 2, 3] },
      ],
    })
    expect(merged([])).toBeUndefined()
  })
  it("ends a range on a whole minute", () => {
    expect(minuteOf(120_059)).toBe(120_000)
  })
})
