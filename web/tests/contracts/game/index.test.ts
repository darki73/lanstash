import { describe, expect, it } from "vitest"
import type { GameDetail } from "@/contracts/game"

describe("contracts/game", () => {
  it("describes a game with its builds and their figures", () => {
    const g: GameDetail = {
      game: "osi",
      product: "osi",
      region: "eu",
      build: {
        version: "3.3",
        build_config: "ab",
        have: 1,
        want: 2,
        complete: false,
        plan_error: null,
        detail: {
          files_have: 1,
          files_want: 1,
          archived_have: 0,
          archived_want: 1,
          file_count: 3,
          part_count: 4,
          archive_count: 1,
          requests: 5,
          absent: 0,
        },
      },
      prerelease: null,
      error: null,
    }
    expect(g.build?.detail?.part_count).toBe(4)
  })
})
