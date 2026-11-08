import { describe, expect, it } from "vitest"
import { eventsOf, type HistoryEvent, noHistory } from "@/contracts/history"

const event = (platform: string, game: string, at_ms: number): HistoryEvent => ({
  at_ms,
  platform,
  game,
  change: "current",
  version: "1",
  from: null,
})

describe("contracts/history", () => {
  it("starts empty", () => {
    expect(noHistory.events).toEqual([])
  })
  it("picks one game's events, in their order", () => {
    const history = { events: [event("steam", "7", 3), event("steam", "8", 2), event("blizzard", "7", 1)] }
    expect(eventsOf(history, "steam", "7")).toEqual([event("steam", "7", 3)])
    expect(eventsOf(history, "epicgames", "7")).toEqual([])
  })
})
