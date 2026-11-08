import { describe, expect, it } from "vitest"
import { noSetup } from "@/contracts/setup"

describe("contracts/setup", () => {
  it("has a link that offers no file", () => {
    expect(noSetup("epicgames", "windows")).toBeUndefined()
  })
})
