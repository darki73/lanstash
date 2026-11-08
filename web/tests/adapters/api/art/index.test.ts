import { describe, expect, it } from "vitest"
import { artLink } from "@/adapters/api/art"

describe("adapters/api/art", () => {
  it("puts the API base, the platform and the escaped id together", () => {
    expect(artLink("")("steam", "1422450")).toBe("/api/v1/art/steam/1422450")
    expect(artLink("http://box")("steam", "a/b")).toBe("http://box/api/v1/art/steam/a%2Fb")
    expect(artLink("")("blizzard", "Hero-Live-a", "icon")).toBe("/api/v1/art/blizzard/Hero-Live-a/icon")
    expect(artLink("")("blizzard", "osi", "header")).toBe("/api/v1/art/blizzard/osi")
    expect(artLink("")("blizzard", "wow_classic", "background")).toBe("/api/v1/art/blizzard/wow_classic/background")
    expect(artLink("")("steam", "1085660", "logo")).toBe("/api/v1/art/steam/1085660/logo")
  })
})
