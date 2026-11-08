import { describe, expect, it } from "vitest"
import type { Prefill } from "@/contracts/prefill"
import { noRuns, took } from "@/contracts/runs"

const job = (started_ms: number, finished_ms: number | null): Prefill => ({
  game: "7",
  version: "1",
  state: finished_ms === null ? "running" : "done",
  files_total: 1,
  files_done: 1,
  files_skipped: 0,
  files_missing: 0,
  bytes: 1,
  current: null,
  error: null,
  started_ms,
  finished_ms,
})

describe("contracts/runs", () => {
  it("says how long a run took, once it ended", () => {
    expect(took(job(1_000, 61_000))).toBe(60_000)
    expect(took(job(1_000, null))).toBeUndefined()
  })
  it("has a reader for a page with none", async () => {
    expect(await noRuns("steam")).toBeUndefined()
  })
})
