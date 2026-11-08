import { describe, expect, it } from "vitest"
import { noArt } from "@/contracts/art"

describe("contracts/art", () => {
  it("has a link that shows no art", () => {
    expect(noArt("steam", "1422450")).toBeUndefined()
  })
})
