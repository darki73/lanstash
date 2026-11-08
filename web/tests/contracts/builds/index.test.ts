import { describe, expect, it } from "vitest"
import { buildOf } from "@/contracts/builds"

const osi = { game: "osi", product: "osi", version: "3.3", build_config: "ab", current: true, error: null }

describe("contracts/builds", () => {
  it("finds a game's build, if any", () => {
    expect(buildOf({ region: "eu", games: [osi] }, "osi")).toBe(osi)
    expect(buildOf({ region: "eu", games: [osi] }, "hs")).toBeUndefined()
    expect(buildOf(undefined, "osi")).toBeUndefined()
  })
})
