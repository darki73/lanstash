import { describe, expect, it } from "vitest"
import { toneBg, toneText } from "@/styled/tone"

describe("styled/tone", () => {
  it("has a text and a background colour for every tone", () => {
    expect(Object.keys(toneText).sort()).toEqual(["cache", "origin", "plain"])
    expect(Object.keys(toneBg).sort()).toEqual(Object.keys(toneText).sort())
    expect([toneText.cache, toneBg.origin]).toEqual(["text-cache", "bg-origin"])
  })
})
