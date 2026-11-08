import { describe, expect, it } from "vitest"
import { fold, match } from "@/lib/match"

describe("lib/match", () => {
  it("ignores case, accents and punctuation", () => {
    expect(fold("  Pokémon: Let's GO!  ")).toBe("pokemon let s go")
    expect(match("Pokémon", "pokemon")).toBe(4)
  })
  it("ranks a start over a word over anywhere over initials", () => {
    expect(match("Diablo IV", "dia")).toBe(4)
    expect(match("Grand Theft Auto V", "theft")).toBe(3)
    expect(match("Overwatch", "watch")).toBe(2)
    expect(match("Grand Theft Auto V", "gta")).toBe(1)
    expect(match("Grand Theft Auto V", "g t a")).toBe(1)
  })
  it("has no score for a name that does not answer, and the same for all when nothing is typed", () => {
    expect(match("Destiny 2", "zelda")).toBeUndefined()
    expect(match("Destiny 2", "  ")).toBe(0)
  })
})
