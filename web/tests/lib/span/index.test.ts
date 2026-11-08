import { describe, expect, it } from "vitest"
import { span } from "@/lib/span"

describe("lib/span", () => {
  it("measures a length of time in the unit that reads well", () => {
    expect([span(1), span(45), span(120), span(60 * 60), span(5400)]).toEqual([
      { unit: "seconds", count: 1 },
      { unit: "seconds", count: 45 },
      { unit: "minutes", count: 2 },
      { unit: "minutes", count: 60 },
      { unit: "hours", count: 1.5 },
    ])
  })
})
