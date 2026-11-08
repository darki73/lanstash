import { render, renderHook, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { HistoryEvent } from "@/contracts/history"
import { dayOf, days, HistoryView, steps, timeOf } from "@/features/history"
import { useT } from "@/i18n"
import { snapshot } from "../../kit"

const t = renderHook(() => useT("history")).result.current

const at = (y: number, mo: number, d: number, h: number, mi: number) => new Date(y, mo - 1, d, h, mi).getTime()
const now = at(2026, 11, 7, 17, 30)

const event = (over: Partial<HistoryEvent>): HistoryEvent => ({
  at_ms: at(2026, 11, 7, 14, 5),
  platform: "epicgames",
  game: "ad85",
  change: "current",
  version: "v2",
  from: null,
  ...over,
})

const usage = [
  { service: "epicgames", game: "a8e6", name: "Railway Game", group: "ad85", objects: 3, bytes: 9, last_used_ms: 0 },
]

describe("features/history", () => {
  it("names days and times in the viewer's time zone", () => {
    expect(timeOf(at(2026, 11, 7, 9, 4))).toBe("09:04")
    expect(dayOf(at(2026, 11, 7, 0, 1), now, t)).toBe("Today")
    expect(dayOf(at(2026, 11, 6, 23, 59), now, t)).toBe("Yesterday")
    expect(dayOf(at(2026, 11, 3, 12, 0), now, t)).toBe("Tue 3 Nov")
    expect(dayOf(at(2025, 11, 3, 12, 0), now, t)).toBe("Mon 3 Nov 2025")
  })
  it("groups the events by day, names their games by the stash, else the catalog, else the id", () => {
    const history = {
      events: [
        event({}),
        event({ change: "new-build", from: "v1", at_ms: at(2026, 11, 7, 9, 0) }),
        event({ platform: "steam", game: "7", change: "added", version: "12", at_ms: at(2026, 11, 6, 20, 0) }),
        event({ platform: "steam", game: "8", version: "3", at_ms: at(2026, 11, 6, 19, 0) }),
      ],
    }
    const catalogs = {
      steam: { products: [{ code: "7", name: "Seven", game: "7", version: "12", known: true }] },
    }
    const grouped = days({ history, snapshot: snapshot({ now_ms: now, usage }), catalogs }, t, "A hidden game")
    expect(grouped.map((d) => [d.label, d.entries.length])).toEqual([
      ["Today", 1],
      ["Yesterday", 2],
    ])
    const [first] = grouped[0]?.entries ?? []
    expect(first).toMatchObject({ title: "Railway Game", href: "/platforms/epicgames/ad85", kind: "done" })
    expect(first?.subtitle).toBe("Epic Games")
    expect(first, "the new build and its current: one line, at the time it became current").toMatchObject({
      label: "new build, now current",
      time: "14:05",
      detail: "v2",
      before: "v1",
    })
    expect(grouped[1]?.entries.map((e) => [e.title, e.kind])).toEqual([
      ["Seven", "seen"],
      ["8", "done"],
    ])
  })
  it("folds a current only into the same game's build just before it", () => {
    const e = [
      event({ game: "a", change: "current", version: "2" }),
      event({ game: "a", change: "added", version: "1" }),
      event({ game: "b", change: "current", version: "5" }),
      event({ game: "c", change: "added", version: "5" }),
      event({ game: "d", change: "current", version: "7" }),
      event({ game: "d", change: "added", version: "7" }),
    ]
    expect(steps(e).map((s) => [s.event.game, s.event.change, s.finished?.change])).toEqual([
      ["a", "current", undefined],
      ["a", "added", undefined],
      ["b", "current", undefined],
      ["c", "added", undefined],
      ["d", "added", "current"],
    ])
  })
  it("masks a hidden game's name, page and builds", () => {
    const [day] = days(
      {
        history: { events: [event({ change: "new-build", version: "26510590", from: "24061265" })] },
        snapshot: snapshot({ now_ms: now, usage }),
        privacy: (p, g) => (p === "epicgames" && g === "ad85" ? "masked" : undefined),
      },
      t,
      "A hidden game",
    )
    const entry = day?.entries[0]
    expect([entry?.title, entry?.href, entry?.detail, entry?.before, entry?.subtitle]).toEqual([
      "A hidden game",
      undefined,
      "",
      undefined,
      "Epic Games",
    ])
  })
  it("says when nothing was updated yet", () => {
    render(<HistoryView history={{ events: [] }} snapshot={snapshot()} />)
    expect(screen.getByText("Nothing updated yet")).toBeTruthy()
  })
  it("draws the timeline", () => {
    render(<HistoryView history={{ events: [event({})] }} snapshot={snapshot({ now_ms: now, usage })} />)
    expect(screen.getByRole("link", { name: "Railway Game" })).toBeTruthy()
    expect(screen.getByText("current")).toBeTruthy()
  })
})
