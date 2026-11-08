import { describe, expect, it } from "vitest"
import { linkOptions, toggleOf, toneOf } from "@/features/access/roles"

describe("features/access/roles", () => {
  it("gives each role its tone", () => {
    expect([toneOf("viewer"), toneOf("player"), toneOf("admin")]).toEqual(["least", "middle", "strong"])
  })
  it("shows a platform account with its platform first", () => {
    expect(toggleOf("steam/some/one")).toEqual({ id: "steam/some/one", group: "steam", label: "some/one" })
    expect(toggleOf("plain")).toEqual({ id: "plain", label: "plain" })
  })
  it("offers the accounts lanstash has, and any kept beside them", () => {
    expect(linkOptions(["steam/a"], ["steam/a", "steam/gone"]).map((o) => o.id)).toEqual(["steam/a", "steam/gone"])
  })
})
