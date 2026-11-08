import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { Deleting } from "@/contracts/stash"
import { DeletingPanel } from "@/features/platforms/deleting"

const job = (over: Partial<Deleting>): Deleting => ({
  platform: "blizzard",
  game: "sc1live",
  state: "running",
  items_total: 8,
  items_done: 3,
  bytes_total: 5_780_000_000,
  bytes_done: 2_000_000_000,
  failed: 0,
  started_ms: 0,
  finished_ms: null,
  ...over,
})

describe("features/platforms/deleting", () => {
  it("shows how much of a game is gone while it is being deleted", () => {
    render(<DeletingPanel job={job({})} name="StarCraft" now={6_000} />)
    expect(screen.getByRole("region", { name: "Deleting · StarCraft" })).toBeTruthy()
    expect(screen.getByText("2.00 GB of 5.78 GB")).toBeTruthy()
    expect(screen.getByText("3/8")).toBeTruthy()
    expect(screen.getByText("6 seconds")).toBeTruthy()
    expect(screen.queryByRole("link")).toBeNull()
  })
  it("says Pruning for a prune", () => {
    render(<DeletingPanel job={job({ partial: true })} name="WoW" now={0} />)
    expect(screen.getByRole("region", { name: "Pruning · WoW" })).toBeTruthy()
  })
  it("says what went once done, what could not, and leads back", () => {
    const done = job({ state: "done", items_done: 8, bytes_done: 5_780_000_000, failed: 1, finished_ms: 14_000 })
    render(
      <DeletingPanel job={done} name="StarCraft" now={60_000} back={{ href: "/platforms/blizzard", label: "Back" }} />,
    )
    expect(screen.getByRole("region", { name: "Deleted · StarCraft" })).toBeTruthy()
    expect(screen.getByText("5.78 GB of 5.78 GB")).toBeTruthy()
    expect(screen.getByText("14 seconds")).toBeTruthy()
    expect(screen.getByText("could not be deleted").previousSibling?.textContent).toBe("1")
    expect(screen.getByRole("link", { name: "Back" }).getAttribute("href")).toBe("/platforms/blizzard")
  })
})
