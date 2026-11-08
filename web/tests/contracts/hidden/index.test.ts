import { describe, expect, it } from "vitest"
import { hiddenKey, noHiding, privacyOf } from "@/contracts/hidden"

describe("contracts/hidden", () => {
  it("masks a hidden game, reveals it to a browser that shows hidden games, and leaves the rest", () => {
    const hidden = [hiddenKey("steam", "10")]
    expect(privacyOf(hidden, false, "steam", "10")).toBe("masked")
    expect(privacyOf(hidden, true, "steam", "10")).toBe("revealed")
    expect(privacyOf(hidden, false, "steam", "11")).toBeUndefined()
    expect(privacyOf(null, false, "steam", "10")).toBeUndefined()
  })
  it("cannot hide anything without lanstash", async () => {
    expect(await noHiding.set("steam", "1", true)).toBe("lanstash keeps no hidden games")
  })
})
