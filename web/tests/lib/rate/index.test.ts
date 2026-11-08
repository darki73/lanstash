import { describe, expect, it } from "vitest"
import { rate } from "@/lib/rate"

describe("lib/rate", () => {
  it("says a rate, or a dash when there is none yet", () => {
    expect(rate(389.96)).toBe("390.0 MB/s")
    expect(rate(undefined)).toBe("–")
  })
})
