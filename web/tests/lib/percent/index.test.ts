import { describe, expect, it } from "vitest"
import { done } from "@/lib/percent"

describe("done", () => {
  it("rounds down so only all of it is 100 %", () => {
    expect(done(999, 1000)).toBe("99 %")
    expect(done(1000, 1000)).toBe("100 %")
    expect(done(2000, 1000)).toBe("100 %")
    expect(done(415, 1000)).toBe("41 %")
  })

  it("says when it has barely started or there is no whole", () => {
    expect(done(1, 1000)).toBe("< 1 %")
    expect(done(0, 1000)).toBe("0 %")
    expect(done(5, 0)).toBe("–")
  })
})
