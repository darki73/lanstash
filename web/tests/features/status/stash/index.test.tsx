import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Fullness, Library, StashPanel } from "@/features/status/stash"
import { snapshot } from "../../../kit"

const usage = [
  { service: "steam", game: "11", name: "Game A", group: "1", objects: 1, bytes: 3e9 },
  { service: "steam", game: "12", name: "Game A", group: "1", objects: 1, bytes: 1e9 },
  ...Array.from({ length: 13 }, (_, i) => ({
    service: "steam",
    game: `${100 + i}`,
    name: `Other ${i}`,
    group: `${100 + i}`,
    objects: 1,
    bytes: 1e6,
  })),
  { service: "blizzard", game: "wow", name: "World", objects: 1, bytes: 2e9 },
  { service: "blizzard", game: "configs", name: null, objects: 1, bytes: 5e6 },
]
const s = snapshot({ usage, counters: { eviction: { stored_bytes: 6e9, disk_free_bytes: 4e9 } } })

describe("features/status/stash", () => {
  it("shows how full the store is, by platform", () => {
    render(<Fullness snapshot={s} />)
    expect(screen.getByText("6.00 GB stored")).toBeTruthy()
    expect(screen.getByText("4.00 GB free of 10.00 GB")).toBeTruthy()
    expect(screen.getByText("Steam")).toBeTruthy()
    expect(screen.getByText("Battle.net")).toBeTruthy()
  })
  it("lists the largest games of all platforms, depots as one game, linking to them", () => {
    render(<StashPanel snapshot={s} />)
    expect(screen.getByRole("link", { name: "Game A" }).getAttribute("href")).toBe("/platforms/steam/1")
    expect(screen.getByText("4.00 GB")).toBeTruthy()
    expect(screen.getByText("8 of 15")).toBeTruthy()
    expect(screen.getByRole("link", { name: "All games by platform" })).toBeTruthy()
  })
  it("lists each platform's largest games, says its launcher data in the heading, with a link to all", () => {
    render(<Library snapshot={s} />)
    expect(screen.getByText("1 game · 5.0 MB launcher data")).toBeTruthy()
    expect(screen.queryByText("Launcher data")).toBeNull()
    expect(screen.getByRole("link", { name: "All 14 games on Steam" }).getAttribute("href")).toBe("/platforms/steam")
    expect(screen.queryByRole("link", { name: /games on Battle.net/ })).toBeNull()
  })
  it("says when nothing is stored", () => {
    render(<StashPanel snapshot={snapshot()} />)
    expect(screen.getByText("Nothing stored yet")).toBeTruthy()
  })
})
