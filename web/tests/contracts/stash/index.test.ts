import { describe, expect, it } from "vitest"
import { type Deleting, deletedShownMs, noStash, recentDeletion } from "@/contracts/stash"

const job = (over: Partial<Deleting>): Deleting => ({
  platform: "blizzard",
  game: "sc1live",
  state: "done",
  items_total: 1,
  items_done: 1,
  bytes_total: 1,
  bytes_done: 1,
  failed: 0,
  started_ms: 0,
  finished_ms: 1_000,
  ...over,
})

describe("contracts/stash", () => {
  it("a page without stash actions says it cannot delete", async () => {
    expect(await noStash.remove("blizzard", "sc1live")).toBe("deleting is not available here")
  })
  it("shows a deletion while it runs, and for a while after", () => {
    expect(recentDeletion(null, 0)).toBeUndefined()
    expect(recentDeletion(job({ state: "running", finished_ms: null }), 1e12)).toBeDefined()
    expect(recentDeletion(job({}), 1_000 + deletedShownMs - 1)).toBeDefined()
    expect(recentDeletion(job({}), 1_000 + deletedShownMs)).toBeUndefined()
  })
})
