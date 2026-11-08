import { describe, expect, it } from "vitest"
import { HIDDEN_NAME } from "@/contracts/hidden"
import { markHidden } from "@/features/platforms/hidden"
import { dirGame, platform } from "@/features/platforms/model"

const usage = [
  { service: "steam", game: "11", group: "10", name: "Secret Game", objects: 1, bytes: 5 },
  { service: "steam", game: "12", group: "10", name: "Secret Game", objects: 1, bytes: 6 },
  { service: "steam", game: "21", group: "20", name: "Open Game", objects: 1, bytes: 7 },
  { service: "blizzard", game: "wow", name: "World of Warcraft", objects: 1, bytes: 8 },
]

describe("features/platforms/hidden", () => {
  it("masks every entry of a hidden game, by its app or its own code", () => {
    const marked = markHidden(usage, ["steam/10", "blizzard/wow"], false)
    expect(marked.map((u) => [u.name, u.privacy])).toEqual([
      [HIDDEN_NAME, "masked"],
      [HIDDEN_NAME, "masked"],
      ["Open Game", undefined],
      [HIDDEN_NAME, "masked"],
    ])
  })
  it("marks without masking for a browser that shows hidden games, and leaves all alone with none hidden", () => {
    expect(markHidden(usage, ["steam/10"], true).map((u) => u.privacy)).toEqual([
      "revealed",
      "revealed",
      undefined,
      undefined,
    ])
    expect(markHidden(usage, null, false)).toEqual(usage)
  })
  it("gives a masked game no art, page or build, and keeps its size", () => {
    const steam = platform(markHidden(usage, ["steam/10"], false), "steam")
    const g = steam?.games.find((x) => x.id === "10")
    expect([g?.name, g?.privacy, g?.art, g?.build, g?.bytes]).toEqual([HIDDEN_NAME, "masked", undefined, undefined, 11])
    const dir = dirGame(markHidden(usage, ["steam/10"], false), "steam", "11", "launcher")
    expect([dir.name, dir.href, dir.art]).toEqual([HIDDEN_NAME, undefined, undefined])
    expect(dirGame(usage, "steam", "11", "launcher").href).toBe("/platforms/steam/10")
  })
  it("masks what lanstash masked itself, whatever the list", () => {
    const sent = [
      {
        service: "steam",
        game: "hidden-1",
        group: "hidden-1",
        name: "A hidden game",
        objects: 1,
        bytes: 9,
        hidden: true,
      },
    ]
    expect(markHidden(sent, null, true).map((u) => [u.name, u.privacy])).toEqual([[HIDDEN_NAME, "masked"]])
  })
  it("never groups two masked games as one", () => {
    const parts = [
      { service: "steam", game: "31", name: "A", objects: 1, bytes: 1 },
      { service: "steam", game: "41", name: "B", objects: 1, bytes: 1 },
    ]
    const steam = platform(markHidden(parts, ["steam/31", "steam/41"], false), "steam")
    expect(steam?.games).toHaveLength(2)
  })
})
