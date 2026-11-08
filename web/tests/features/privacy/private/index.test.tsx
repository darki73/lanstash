import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { PrivateActions } from "@/contracts/private"
import { PrivatePanel } from "@/features/privacy/private"

/** A player's list, kept in memory. */
function actions(start: readonly string[]): PrivateActions & { readonly list: () => readonly string[] } {
  let list = start
  return {
    list: () => list,
    read: async () => list,
    set: async (platform, game, on) => {
      const key = `${platform}/${game}`
      list = on ? [...list, key] : list.filter((g) => g !== key)
      return list
    },
  }
}

describe("features/privacy/private", () => {
  it("keeps a game of one's own private, and takes it back", async () => {
    const a = actions([])
    render(<PrivatePanel platform="steam" game="10" actions={a} />)
    fireEvent.click(await screen.findByRole("button", { name: "Keep private" }))
    expect(await screen.findByRole("button", { name: "Stop keeping private" })).toBeTruthy()
    expect(a.list()).toEqual(["steam/10"])
    expect(screen.getByText(/others don't see that you have it/)).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Stop keeping private" }))
    expect(await screen.findByRole("button", { name: "Keep private" })).toBeTruthy()
    expect(a.list()).toEqual([])
  })
  it("says why lanstash refused, and shows nothing while it cannot say", async () => {
    const refusing: PrivateActions = { read: async () => [], set: async () => "only a game of one's own" }
    render(<PrivatePanel platform="steam" game="20" actions={refusing} />)
    fireEvent.click(await screen.findByRole("button", { name: "Keep private" }))
    expect(await screen.findByText("only a game of one's own")).toBeTruthy()
    const { container } = render(
      <PrivatePanel platform="steam" game="20" actions={{ ...refusing, read: async () => undefined }} />,
    )
    expect(container.textContent).toBe("")
  })
})
