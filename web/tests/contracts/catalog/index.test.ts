import { describe, expect, it } from "vitest"
import { type CatalogProduct, notCached, ownersLine } from "@/contracts/catalog"

const p = (code: string, name: string | null, game: string | null, version: string | null): CatalogProduct => ({
  code,
  name,
  game,
  version,
  known: true,
})

describe("contracts/catalog", () => {
  it("offers the games not cached, one per directory, by name", () => {
    const catalog = {
      products: [
        p("s1", "StarCraft", "sc1live", "1.23"),
        p("d3", "Diablo III", "diablo3", "2.8"),
        p("d3cn", "Diablo III", "diablo3", null),
        p("osi", "Diablo II: Resurrected", "osi", "3.3"),
        p("btlr", "Black Ops 6", null, "3.0"),
        p("cornv1", "Corn", "cornv", null),
        p("w2bn", null, "w2bn", "2.0"),
      ],
    }
    expect(notCached(catalog, new Set(["osi"])).map((o) => o.game)).toEqual(["diablo3", "sc1live"])
    const sized = { products: [{ ...p("7", "G", "7", "9"), download: 100 }] }
    expect(notCached(sized, new Set())[0]?.download, "the download size is kept").toBe(100)
    expect(notCached(undefined, new Set())).toEqual([])
  })
  it("says who owns a game only where the games have more than one owner between them", () => {
    const owned = (code: string, owners: string[]) => ({ ...p(code, `G${code}`, code, "1"), owners })
    const two = notCached(
      { products: [owned("1", ["player_one"]), owned("2", ["player_one", "player_two"])] },
      new Set(),
    )
    const line = ownersLine(two)
    expect(two.map(line)).toEqual(["player_one", "player_one, player_two"])
    const one = notCached({ products: [owned("1", ["player_one"]), owned("2", ["player_one"])] }, new Set())
    expect(one.map(ownersLine(one)), "one account: nothing to say").toEqual([undefined, undefined])
  })
})
