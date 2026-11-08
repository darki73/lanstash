import { render, renderHook, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { behindLabel, PlatformView } from "@/features/platforms"
import { useT } from "@/i18n"
import { snapshot } from "../../kit"

const usage = [
  { service: "blizzard", game: "osi", name: "Diablo II: Resurrected", objects: 669, bytes: 29_620_000_000 },
  { service: "blizzard", game: "catalogs", name: null, objects: 8, bytes: 287_000 },
  { service: "steam", game: "1422456", name: "Deadlock", objects: 17_766, bytes: 6_120_000_000 },
]

describe("features/platforms", () => {
  it("lays out a platform's page: figures, its parts, and the cached games first", () => {
    const downloads = [{ key: "blizzard/tpr/osi/data/x", span: null, received: 1, total: null }]
    const { rerender } = render(<PlatformView snapshot={snapshot({ usage, downloads })} id="blizzard" />)
    expect(screen.getByRole("heading", { level: 2, name: "Battle.net" })).toBeTruthy()
    expect(screen.getByText("Blizzard's games, from Blizzard's CDN")).toBeTruthy()
    expect(screen.getByText("83 %")).toBeTruthy()
    const parts = screen.getByRole("navigation", { name: "Battle.net's parts" })
    expect([...parts.querySelectorAll("a")].map((a) => a.getAttribute("href"))).toEqual([
      "/platforms/blizzard",
      "/platforms/blizzard/library",
      "/platforms/blizzard/prefill",
      "/platforms/blizzard/activity",
    ])
    expect(screen.getByRole("link", { name: /Cached/ }).getAttribute("aria-current")).toBe("page")
    expect(screen.getByText("1 game in the cache")).toBeTruthy()
    expect(screen.getByText("Diablo II: Resurrected")).toBeTruthy()
    expect(screen.getByText("Launcher data")).toBeTruthy()
    rerender(<PlatformView snapshot={snapshot({ usage, downloads })} id="blizzard" section="activity" />)
    expect(screen.getByText("1 object")).toBeTruthy()
    expect(screen.getByRole("heading", { name: "About Battle.net in lanstash" })).toBeTruthy()
    rerender(<PlatformView snapshot={snapshot({ usage })} id="blizzard" section="account" />)
    expect(screen.getByText("1 game in the cache"), "no such part: the cached games").toBeTruthy()
  })
  it("shows the platform's own job, a line about it over the games, and queues starts behind it", () => {
    const steamJob = {
      game: "1422456",
      version: "1",
      state: "running" as const,
      files_total: 4,
      files_done: 1,
      files_skipped: 0,
      files_missing: 0,
      bytes: 1,
      current: null,
      error: null,
      started_ms: 0,
      finished_ms: null,
      queued: ["7"],
    }
    const snap = snapshot({ usage, prefill: steamJob, prefills: { steam: steamJob } })
    const { rerender } = render(<PlatformView snapshot={snap} id="blizzard" section="prefill" />)
    expect(screen.getByText("No prefill yet"), "Steam's job is not Battle.net's").toBeTruthy()
    rerender(<PlatformView snapshot={snap} id="steam" />)
    expect(screen.getByRole("region", { name: "Prefill · 1 more queued · Deadlock" })).toBeTruthy()
    expect(screen.getByRole("link", { name: /Prefill/ }).textContent).toContain("25 %")
    rerender(<PlatformView snapshot={snap} id="steam" section="prefill" />)
    expect(screen.getByRole("heading", { name: "Up next (1)" })).toBeTruthy()
  })
  it("counts as behind only the games known not to be all stashed", () => {
    const more = [
      ...usage,
      { service: "blizzard", game: "hs", name: "Hearthstone", objects: 1, bytes: 1 },
      { service: "blizzard", game: "pro", name: "Overwatch", objects: 1, bytes: 1 },
    ]
    const game = (game: string, have: number | null, want: number | null) => ({
      game,
      product: game,
      version: "1",
      build_config: "a",
      have,
      want,
      current: have !== null && have === want,
      error: null,
    })
    const builds = { region: "eu", games: [game("osi", 5, 5), game("hs", 1, 9), game("pro", null, null)] }
    const { rerender } = render(<PlatformView snapshot={snapshot({ usage: more })} id="blizzard" builds={builds} />)
    expect(screen.getByText("behind in EU").previousSibling?.textContent).toBe("1")
    expect(screen.queryByText("downloading"), "nothing downloading: no chip").toBeNull()
    const caughtUp = { region: "eu", games: [game("osi", 5, 5), game("hs", 9, 9)] }
    rerender(<PlatformView snapshot={snapshot({ usage: more })} id="blizzard" builds={caughtUp} />)
    expect(screen.queryByText("behind in EU")).toBeNull()
    expect(screen.getByText("current").previousSibling?.textContent).toBe("all")
    const t = renderHook(() => useT("platforms")).result.current
    expect(behindLabel(t, "os", "windows")).toBe("behind for Windows")
  })
  it("says how games get there when nothing is cached, and when there is no such platform", () => {
    const { unmount } = render(<PlatformView snapshot={snapshot()} id="riot" />)
    expect(screen.getByText("Nothing from Riot Games in the cache yet")).toBeTruthy()
    expect(screen.queryByRole("heading", { name: /^About/ })).toBeNull()
    unmount()
    render(<PlatformView snapshot={snapshot()} id="nothing" />)
    expect(screen.getByText("No platform called nothing")).toBeTruthy()
  })
})
