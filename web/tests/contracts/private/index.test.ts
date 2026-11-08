import { describe, expect, it } from "vitest"
import { noPrivate } from "@/contracts/private"

describe("contracts/private", () => {
  it("keeps nothing private without lanstash", async () => {
    expect(await noPrivate.read()).toBeUndefined()
    expect(await noPrivate.set("steam", "1", true)).toBe("lanstash keeps no private games")
  })
})
