import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { SeriesQuery, StatsReader } from "@/contracts/stats"
import { KEPT_NAMES, Overview } from "@/features/status"
import { ranges } from "@/features/status/range"
import { resources } from "@/features/status/resource"
import { live } from "../../kit"

const recorded = () => {
  const asked: SeriesQuery[] = []
  const stats: StatsReader = async (q) => {
    asked.push(q)
    return { step_ms: q.step_ms, names: q.names, buckets: [] }
  }
  return { asked, stats }
}

describe("features/status", () => {
  it("draws the home under the status line and a tile per resource, none marked", async () => {
    const { stats } = recorded()
    render(<Overview live={live()} stats={stats} />)
    const tiles = screen.getByRole("navigation", { name: "Resources" })
    const links = [...tiles.querySelectorAll("a")]
    expect(links.map((a) => a.getAttribute("href"))).toEqual(resources.map((r) => `/overview/${r}`))
    expect(links.every((a) => a.getAttribute("aria-current") === null)).toBe(true)
    expect(screen.getByRole("region", { name: "Status" })).toBeTruthy()
    expect(await screen.findByRole("region", { name: "Traffic" })).toBeTruthy()
  })
  it("draws a resource's page with its tile marked", () => {
    render(<Overview live={live()} resource="storage" />)
    const links = [...screen.getByRole("navigation", { name: "Resources" }).querySelectorAll("a")]
    expect(links.map((a) => a.getAttribute("aria-current"))).toEqual([null, null, null, "page", null])
    expect(screen.getByRole("heading", { level: 2, name: "Storage" })).toBeTruthy()
  })
  it("draws each resource's page", () => {
    for (const [resource, name] of [
      ["throughput", "Throughput"],
      ["downloads", "Downloads"],
      ["requests", "Requests"],
      ["connections", "Connections"],
    ] as const) {
      const { unmount } = render(<Overview live={live()} resource={resource} />)
      expect(screen.getByRole("heading", { level: 2, name })).toBeTruthy()
      unmount()
    }
  })
  it("asks for every counter it draws over the range chosen, in asks lanstash takes", async () => {
    const { asked, stats } = recorded()
    const l = live()
    render(<Overview live={l} stats={stats} />)
    await waitFor(() => expect(asked.length).toBeGreaterThan(0))
    expect(asked.every((q) => q.names.length <= 32)).toBe(true)
    const names = new Set(asked.flatMap((q) => q.names))
    expect(KEPT_NAMES.every((n) => names.has(n))).toBe(true)
    expect(asked[0]).toMatchObject({ step_ms: ranges.day.step })
    fireEvent.click(screen.getByRole("button", { name: "Week" }))
    await waitFor(() => expect(asked.some((q) => q.step_ms === ranges.week.step)).toBe(true))
  })
})
