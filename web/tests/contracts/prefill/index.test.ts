import { describe, expect, it } from "vitest"
import { noPrefill, type Prefill, running } from "@/contracts/prefill"

const job = (state: Prefill["state"]): Prefill => ({
  game: "hs",
  version: "1",
  state,
  files_total: 2,
  files_done: 1,
  files_skipped: 0,
  files_missing: 0,
  bytes: 5,
  current: null,
  error: null,
  started_ms: 1,
  finished_ms: null,
})

describe("contracts/prefill", () => {
  it("knows a running prefill", () => {
    expect([running(job("running")), running(job("done")), running(null), running(undefined)]).toEqual([
      true,
      false,
      false,
      false,
    ])
  })
  it("has actions for a page that cannot prefill", async () => {
    expect(await noPrefill.start("blizzard", "hs")).toBe("prefill is not available here")
    await noPrefill.stop()
  })
})
