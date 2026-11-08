import { renderHook } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { HOUR, per, rangeChoices, rangeOptions, ranges, spanOf } from "@/features/status/range"
import { useT } from "@/i18n"

const t = () => renderHook(() => useT("status")).result.current

describe("features/status/range", () => {
  it("offers a day, a week and a month, each in at most a few hundred buckets", () => {
    expect(rangeOptions.map((o) => o.id)).toEqual(["day", "week", "month"])
    for (const r of Object.values(ranges)) expect(r.span / r.step).toBeLessThanOrEqual(200)
  })
  it("says the choices and the spans in words", () => {
    const words = t()
    expect(rangeChoices(words).map((o) => o.label)).toEqual(["Day", "Week", "Month"])
    expect(spanOf(words, "week")).toBe("7 days")
  })
  it("says a bucket's length", () => {
    const words = t()
    expect([per(HOUR / 6, words), per(HOUR, words), per(6 * HOUR, words)]).toEqual(["10 min", "1 h", "6 h"])
  })
})
