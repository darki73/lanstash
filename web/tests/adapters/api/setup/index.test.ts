import { describe, expect, it } from "vitest"
import { setupLink } from "@/adapters/api/setup"

describe("adapters/api/setup", () => {
  it("puts the API base, the escaped platform and the system together", () => {
    expect(setupLink("")("epicgames", "windows")).toBe("/api/v1/setup/epicgames/windows")
    expect(setupLink("http://box")("a/b", "macos")).toBe("http://box/api/v1/setup/a%2Fb/macos")
  })
})
