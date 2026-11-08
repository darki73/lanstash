import { describe, expect, it } from "vitest"
import { count } from "@/lib/count"

describe("lib/count", () => {
  it("separates thousands", () => {
    expect(count(1234567)).toBe("1,234,567")
    expect(count(0)).toBe("0")
  })
})
