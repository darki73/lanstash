import { describe, expect, it } from "vitest"
import { overviewHome, place } from "@/features/frame/route"

describe("features/frame/route", () => {
  it("leads a path to the overview or a platform", () => {
    expect(place("/overview/storage")).toEqual({ kind: "overview", resource: "storage" })
    expect(place("/settings")).toEqual({ kind: "settings" })
    expect(place("/settings/")).toEqual({ kind: "settings" })
    expect(place("/dns")).toEqual({ kind: "dns" })
    expect(place("/history")).toEqual({ kind: "history" })
    expect(place("/health")).toEqual({ kind: "health" })
    expect(place("/clients")).toEqual({ kind: "clients" })
    expect(place("/sign-in")).toEqual({ kind: "sign-in" })
    const token = "0123456789abcdef".repeat(4)
    expect(place(`/join/${token}`)).toEqual({ kind: "join", token })
    expect(place("/join/not-a-token").kind).toBe("overview")
    expect(place(`/join/${token}/more`).kind).toBe("overview")
    expect(place("/account")).toEqual({ kind: "account" })
    expect(place("/accounts")).toEqual({ kind: "accounts" })
    expect(place("/clients/x"), "nothing under it").toEqual({ kind: "overview", resource: undefined })
    expect(place("/history/x"), "nothing under it").toEqual({ kind: "overview", resource: undefined })
    expect(place("/dns/x"), "nothing under it").toEqual({ kind: "overview", resource: undefined })
    expect(place("/platforms/blizzard")).toEqual({ kind: "platform", platform: "blizzard", section: "cached" })
    expect(place("/platforms/blizzard/")).toEqual({ kind: "platform", platform: "blizzard", section: "cached" })
    expect(place("/platforms/steam/library")).toEqual({ kind: "platform", platform: "steam", section: "library" })
    expect(place("/platforms/steam/prefill")).toEqual({ kind: "platform", platform: "steam", section: "prefill" })
    expect(place("/platforms/steam/account")).toEqual({ kind: "platform", platform: "steam", section: "account" })
    expect(place("/platforms/steam/activity")).toEqual({ kind: "platform", platform: "steam", section: "activity" })
    expect(place("/platforms/steam/cached"), "the cached part is the page itself").toEqual({
      kind: "game",
      platform: "steam",
      game: "cached",
    })
    expect(place("/platforms/blizzard/Hero-Live-a")).toEqual({
      kind: "game",
      platform: "blizzard",
      game: "Hero-Live-a",
    })
    expect(place("/platforms/steam/1422450")).toEqual({ kind: "game", platform: "steam", game: "1422450" })
  })
  it("leads anything else to the overview home", () => {
    for (const path of [
      "/",
      "",
      "/overview",
      "/overview/nothing",
      "/platforms",
      "/platforms/<x>",
      "/elsewhere",
      "/platforms/blizzard/a.b",
      "/platforms/blizzard/osi/x",
    ]) {
      expect(place(path)).toEqual({ kind: "overview", resource: undefined })
    }
    expect(overviewHome).toBe("/overview")
  })
})
