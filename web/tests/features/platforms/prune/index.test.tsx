import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { Prunable } from "@/contracts/stash"
import { PrunePanel } from "@/features/platforms/prune"

describe("features/platforms/prune", () => {
  it("checks what old builds take, then prunes it after asking once more", async () => {
    const pruned: string[] = []
    let checks = 0
    const found: Prunable = { files: 12, bytes: 40_000_000_000, recent_bytes: 2_000_000_000 }
    render(
      <PrunePanel
        name="World of Warcraft"
        check={async () => {
          checks += 1
          return checks === 1 ? found : { ...found, bytes: 30_000_000_000 }
        }}
        prune={async () => {
          pruned.push("wow")
          return undefined
        }}
        busy={false}
      />,
    )
    fireEvent.click(screen.getByRole("button", { name: "Check World of Warcraft's old builds" }))
    expect(await screen.findByText(/40.00 GB in 12 files no current build needs/)).toBeTruthy()
    expect(screen.getByText("2.00 GB more is kept for now: used in the last week.")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Prune World of Warcraft's old builds…" }))
    expect(pruned, "asks first").toEqual([])
    // Asked with what a fresh look finds, not the first check's figures.
    const confirm = await screen.findByRole("button", { name: "Prune World of Warcraft's old builds" })
    expect(confirm.textContent).toBe("Prune 30.00 GB")
    fireEvent.click(confirm)
    await screen.findByRole("button", { name: "Check World of Warcraft's old builds" })
    expect(pruned).toEqual(["wow"])
  })
  it("says how many segments are compacted rather than deleted whole", async () => {
    const found: Prunable = { files: 3, bytes: 2_000_000_000, recent_bytes: 0, compact: 2 }
    render(<PrunePanel name="Deadlock" check={async () => found} prune={async () => undefined} busy={false} />)
    fireEvent.click(screen.getByRole("button", { name: "Check Deadlock's old builds" }))
    expect(
      await screen.findByText("2 of them also hold chunks current builds need: those are copied out first."),
    ).toBeTruthy()
  })
  it("says when there is nothing to prune, and why a check failed", async () => {
    const { rerender } = render(
      <PrunePanel
        name="X"
        check={async () => ({ files: 0, bytes: 0, recent_bytes: 0 })}
        prune={async () => undefined}
        busy
      />,
    )
    fireEvent.click(screen.getByRole("button", { name: "Check X's old builds" }))
    expect(await screen.findByText(/Nothing to prune/)).toBeTruthy()
    rerender(<PrunePanel name="Y" check={async () => "no current build"} prune={async () => undefined} busy={false} />)
    fireEvent.click(screen.getByRole("button", { name: "Check Y's old builds" }))
    expect(await screen.findByText("no current build")).toBeTruthy()
  })
})
