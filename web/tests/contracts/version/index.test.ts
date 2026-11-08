import { describe, expect, it } from "vitest"
import { updateAvailable } from "@/contracts/version"

describe("contracts/version", () => {
  it("offers a newer release only when lanstash knows of a different one", () => {
    expect(updateAvailable({ version: "0.1.0" })).toBeUndefined()
    expect(updateAvailable({ version: "0.1.0", latest: null })).toBeUndefined()
    expect(updateAvailable({ version: "0.1.0", latest: "0.1.0" })).toBeUndefined()
    expect(updateAvailable({ version: "0.1.0", latest: "0.2.0" })).toBe("0.2.0")
  })
})
