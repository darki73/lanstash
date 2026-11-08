import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { Prefill } from "@/contracts/prefill"
import type { RunsReader } from "@/contracts/runs"
import { RunsPanel } from "@/features/platforms/runs"

const HOUR = 3_600_000
const now = 100 * HOUR

const job = (game: string, state: Prefill["state"], error: string | null = null): Prefill => ({
  game,
  version: "1",
  state,
  files_total: 10,
  files_done: 10,
  files_skipped: 2,
  bytes_skipped: 3_000_000_000,
  files_missing: 0,
  bytes: 2_000_000_000,
  current: null,
  error,
  started_ms: now - 2 * HOUR,
  finished_ms: now - 2 * HOUR + 90_000,
})

const props = (runs: RunsReader) => ({
  platform: "steam",
  runs,
  ended: null,
  now,
  nameOf: (g: string) => (g === "7" ? "Seven" : g),
  linkOf: (g: string) => `/platforms/steam/${g}`,
})

describe("features/platforms/runs", () => {
  it("lists the platform's past prefills: game, how it ended, bytes, how long, when", async () => {
    const asked: string[] = []
    const runs: RunsReader = async (p) => {
      asked.push(p)
      return [
        { platform: "steam", job: job("7", "done") },
        { platform: "steam", job: job("8", "failed", "access denied") },
      ]
    }
    render(<RunsPanel {...props(runs)} />)
    expect((await screen.findByRole("link", { name: "Seven" })).getAttribute("href")).toBe("/platforms/steam/7")
    expect(screen.getByText("failed").closest("[title]")?.getAttribute("title")).toBe("access denied")
    expect(screen.getAllByText("2.00 GB")).toHaveLength(2)
    expect(screen.getAllByText("3.00 GB")).toHaveLength(2)
    expect(screen.getAllByText("1.5 minutes")).toHaveLength(2)
    expect(screen.getByText("2 kept")).toBeTruthy()
    expect(asked).toEqual(["steam"])
  })
  it("says when none ended yet", async () => {
    render(<RunsPanel {...props(async () => [])} />)
    expect(await screen.findByText("No prefill ended yet")).toBeTruthy()
  })
})
