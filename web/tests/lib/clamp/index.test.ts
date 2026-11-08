import { describe, expect, it } from "vitest"
import { gigabytes, size } from "@/lib/bytes"
import { fraction } from "@/lib/clamp"
import { done } from "@/lib/percent"

describe("lib/clamp", () => {
  it("keeps a fraction between 0 and 1, and what is not a number at 0", () => {
    expect([fraction(0.5), fraction(-1), fraction(3)]).toEqual([0.5, 0, 1])
    expect([fraction(Number.NaN), fraction(Number.POSITIVE_INFINITY)]).toEqual([0, 0])
  })
  it("leaves no NaN in what a page says", () => {
    expect(done(Number.NaN, 10)).toBe("–")
    expect(done(5, Number.POSITIVE_INFINITY)).toBe("–")
    expect(size(Number.NaN)).toBe("–")
    expect(gigabytes(Number.NaN)).toBe("–")
  })
})
