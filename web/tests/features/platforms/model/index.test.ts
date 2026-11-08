import { describe, expect, it } from "vitest"
import {
  dirGame,
  downloadsOf,
  jobOf,
  LAUNCHER_DATA,
  owns,
  platform,
  platformLink,
  platforms,
  sectionLink,
  sortGames,
  worthLancache,
} from "@/features/platforms/model"
import { snapshot } from "../../../kit"

const row = (service: string, game: string, bytes: number, name: string | null = null) => ({
  service,
  game,
  name,
  objects: 2,
  bytes,
})

describe("features/platforms/model", () => {
  it("lists the four known platforms first, then any other service", () => {
    const all = platforms([row("steam", "1", 5), row("wsus", "x", 1), row("blizzard", "osi", 30)])
    expect(all.map((p) => [p.id, p.name, p.brand ?? p.letters])).toEqual([
      ["blizzard", "Battle.net", "battlenet"],
      ["steam", "Steam", "steam"],
      ["epicgames", "Epic Games", "epicgames"],
      ["riot", "Riot Games", "riotgames"],
      ["wsus", "wsus", "WS"],
    ])
    expect(all[2]?.games).toEqual([])
  })
  it("keeps Battle.net's launcher data apart from its games", () => {
    const bn = platform(
      [
        row("blizzard", "osi", 30, "Diablo II: Resurrected"),
        row("blizzard", "catalogs", 1),
        row("blizzard", "drtl", 8),
      ],
      "blizzard",
    )
    expect(bn?.games.map((g) => [g.name, g.named, g.codes])).toEqual([
      ["Diablo II: Resurrected", true, ["osi"]],
      ["drtl", false, ["drtl"]],
    ])
    expect(bn?.systemData.map((s) => s.game)).toEqual(["catalogs"])
    expect([bn?.bytes, bn?.objects]).toEqual([39, 6])
  })
  it("says a store directory's game, the launcher's data being one entry", () => {
    const usage = [
      row("blizzard", "osi", 30, "Diablo II: Resurrected"),
      { ...row("steam", "71", 9, "Seven"), group: "70" },
    ]
    expect(dirGame(usage, "blizzard", "configs", "Launcher data")).toEqual({
      id: LAUNCHER_DATA,
      name: "Launcher data",
      platform: "Battle.net",
      launcher: true,
    })
    expect(dirGame(usage, "blizzard", "bnt004", "x").id).toBe(LAUNCHER_DATA)
    const seven = dirGame(usage, "steam", "71", "x")
    expect([seven.id, seven.name, seven.platform, seven.href, seven.art?.id]).toEqual([
      "70",
      "Seven",
      "Steam",
      "/platforms/steam/70",
      "70",
    ])
    expect(dirGame(usage, "steam", "99", "x"), "nothing of it stashed: the directory").toEqual({
      id: "99",
      name: "99",
      platform: "Steam",
    })
  })
  it("groups Steam's depots under the game's name, a depot without a name on its own", () => {
    const steam = platform(
      [row("steam", "1422450", 1, "Deadlock"), row("steam", "1422456", 40, "Deadlock"), row("steam", "7", 3)],
      "steam",
    )
    expect(steam?.games.map((g) => [g.name, g.codes, g.bytes, g.objects])).toEqual([
      ["Deadlock", ["1422456", "1422450"], 41, 4],
      ["7", ["7"], 3, 2],
    ])
  })
  it("groups Steam's depots by their app when the API says it, and gives the game its art", () => {
    const steam = platform(
      [
        { ...row("steam", "1422451", 9, "Deadlock"), group: "1422450" },
        { ...row("steam", "1422450", 1, "Deadlock (old name)"), group: "1422450" },
        { ...row("steam", "500", 4, "Deadlock"), group: "500" },
        row("steam", "7", 3),
      ],
      "steam",
    )
    expect(steam?.games.map((g) => [g.name, g.codes, g.art])).toEqual([
      ["Deadlock", ["1422451", "1422450"], "1422450"],
      ["Deadlock", ["500"], "500"],
      ["7", ["7"], undefined],
    ])
    expect(
      steam?.games.map((g) => g.id),
      "the app id where the API names it, else the depot",
    ).toEqual(["1422450", "500", "7"])
    const deadlock = steam?.games[0]
    expect(deadlock && [owns(deadlock, "1422450"), owns(deadlock, "1422451"), owns(deadlock, "500")]).toEqual([
      true,
      true,
      false,
    ])
    expect(platform([row("blizzard", "wow", 1, "World of Warcraft")], "blizzard")?.games[0]?.id).toBe("wow")
  })
  it("asks Battle.net's art by the product path, and only for games it can name", () => {
    const bn = platform([row("blizzard", "Hero-Live-a", 2, "Heroes"), row("blizzard", "xyz", 1)], "blizzard")
    expect(bn?.games.map((g) => [g.codes[0], g.art])).toEqual([
      ["Hero-Live-a", "Hero-Live-a"],
      ["xyz", undefined],
    ])
    expect(bn?.art).toEqual({ by: "code", cover: "poster", icon: true })
    expect(platform([row("riot", "x", 1, "X")], "riot")?.games[0]?.art).toBeUndefined()
  })
  it("takes the bnt and bdk directories for launcher data, and gives Battle.net games their build", () => {
    const osi = { game: "osi", product: "osi", version: "3.3", build_config: "a", current: true, error: null }
    const bn = platform(
      [
        row("blizzard", "osi", 5, "D2"),
        row("blizzard", "bnt004", 1),
        row("blizzard", "bnt002", 1),
        row("blizzard", "bdko", 1),
      ],
      "blizzard",
      { region: "eu", games: [osi] },
    )
    expect(bn?.systemData.map((s) => s.game)).toEqual(["bnt004", "bnt002", "bdko"])
    expect(bn?.games.map((g) => [g.codes[0], g.build])).toEqual([["osi", osi]])
    const app = { ...osi, game: "100", product: null }
    const steam = platform([{ ...row("steam", "101", 5, "Some Game"), group: "100" }], "steam", {
      region: "windows",
      games: [app],
    })
    expect(steam?.games[0]?.build, "Steam's build is found by the app id").toEqual(app)
  })
  it("knows a game's last use: the latest of its parts", () => {
    const steam = platform(
      [
        { ...row("steam", "1", 5, "Deadlock"), group: "1", last_used_ms: 50 },
        { ...row("steam", "2", 5, "Deadlock"), group: "1", last_used_ms: 90 },
        row("steam", "3", 1, "Old lanstash"),
      ],
      "steam",
    )
    expect(steam?.games.map((g) => g.lastUsedMs)).toEqual([90, 0])
  })
  it("sorts games by size or by name", () => {
    const games = platform([row("blizzard", "b", 9, "Zeta"), row("blizzard", "a", 1, "Alpha")], "blizzard")?.games ?? []
    expect(sortGames(games, "size").map((g) => g.name)).toEqual(["Zeta", "Alpha"])
    expect(sortGames(games, "name").map((g) => g.name)).toEqual(["Alpha", "Zeta"])
  })
  it("finds a platform's downloads in flight by their key", () => {
    const d = (key: string) => ({ key, span: null, received: 0, total: null })
    expect(downloadsOf([d("blizzard/tpr/x"), d("steam/depot/1"), d("blizzardx/y")], "blizzard")).toEqual([
      d("blizzard/tpr/x"),
    ])
  })
  it("finds a platform by id, and links to it", () => {
    expect(platform([], "riot")?.name).toBe("Riot Games")
    expect(platform([], "nothing")).toBeUndefined()
    expect(platformLink("steam")).toBe("/platforms/steam")
  })
  it("links to each part of a platform's page, the cached games being the page itself", () => {
    expect(sectionLink("steam", "cached")).toBe("/platforms/steam")
    expect(sectionLink("steam", "library")).toBe("/platforms/steam/library")
  })
  it("finds a platform's own job, or the last of any from a lanstash that does not say", () => {
    const job = {
      game: "7",
      version: null,
      state: "done" as const,
      files_total: 0,
      files_done: 0,
      files_skipped: 0,
      files_missing: 0,
      bytes: 0,
      current: null,
      error: null,
      started_ms: 0,
      finished_ms: 1,
    }
    expect(jobOf(snapshot({ prefill: job, prefills: { steam: job } }), "steam")).toBe(job)
    expect(jobOf(snapshot({ prefill: job, prefills: { steam: job } }), "blizzard")).toBeNull()
    expect(jobOf(snapshot({ prefill: job }), "blizzard"), "an older lanstash").toBe(job)
  })
  it("offers lancache's old cache only where it can be read, was probed, and holds more than the stash", () => {
    const share = { have: 5, want: 10 }
    expect(worthLancache(true, share, 2)).toBe(true)
    expect(worthLancache(true, share, null), "nothing stashed").toBe(true)
    expect(worthLancache(false, share, 0), "lanstash cannot read it").toBe(false)
    expect(worthLancache(undefined, share, 0), "an older lanstash").toBe(false)
    expect(worthLancache(true, null, 0), "not probed").toBe(false)
    expect(worthLancache(true, { have: 0, want: 10 }, 0), "lancache holds none of it").toBe(false)
    expect(worthLancache(true, share, 5), "the stash holds as much").toBe(false)
    expect(worthLancache(true, { have: 0, want: 0 }, 0), "nothing to download").toBe(false)
  })
})
