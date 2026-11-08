import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import type { HiddenActions } from "@/contracts/hidden"
import { HiddenGames, HidePanel, PrivacyProvider, privacyValue, usePrivacy } from "@/features/privacy"

const nameOf = (platform: string, game: string) => ({ name: `name of ${game}`, platform: `${platform} platform` })

function Probe() {
  const p = usePrivacy()
  return <output>{String(p.of("steam", "10"))}</output>
}

describe("features/privacy", () => {
  it("says how a game shows, plainly without a provider", () => {
    const { rerender } = render(<Probe />)
    expect(screen.getByRole("status").textContent).toBe("undefined")
    const actions: HiddenActions = { set: async () => [] }
    rerender(
      <PrivacyProvider value={privacyValue(["steam/10"], false, actions)}>
        <Probe />
      </PrivacyProvider>,
    )
    expect(screen.getByRole("status").textContent).toBe("masked")
  })
  it("hides a game from its page, and shows a hidden one to everyone again", async () => {
    const set = vi.fn<HiddenActions["set"]>(async () => ["steam/10"])
    const { rerender } = render(
      <PrivacyProvider value={privacyValue([], false, { set })}>
        <HidePanel platform="steam" game="10" />
      </PrivacyProvider>,
    )
    fireEvent.click(screen.getByText("Hide game"))
    await vi.waitFor(() => expect(set).toHaveBeenCalledWith("steam", "10", true))
    rerender(
      <PrivacyProvider value={privacyValue(["steam/10"], true, { set })}>
        <HidePanel platform="steam" game="10" />
      </PrivacyProvider>,
    )
    fireEvent.click(screen.getByText("Show to everyone"))
    await vi.waitFor(() => expect(set).toHaveBeenCalledWith("steam", "10", false))
  })
  it("shows nothing from a lanstash that keeps no hidden games, and says why a change failed", async () => {
    const { container, rerender } = render(
      <PrivacyProvider value={privacyValue(null, false, { set: async () => "nope" })}>
        <HidePanel platform="steam" game="10" />
        <HiddenGames nameOf={nameOf} />
      </PrivacyProvider>,
    )
    expect(container.textContent).toBe("")
    rerender(
      <PrivacyProvider value={privacyValue([], false, { set: async () => "disk full" })}>
        <HidePanel platform="steam" game="10" />
      </PrivacyProvider>,
    )
    fireEvent.click(screen.getByText("Hide game"))
    expect(await screen.findByText("disk full")).toBeTruthy()
  })
  it("lists the hidden games masked or by name, and switches this browser's choice", () => {
    const setReveal = vi.fn()
    const actions: HiddenActions = { set: async () => [] }
    const { rerender } = render(
      <PrivacyProvider value={privacyValue(["steam/10"], false, actions, setReveal)}>
        <HiddenGames nameOf={nameOf} />
      </PrivacyProvider>,
    )
    expect(screen.getByText("A hidden game")).toBeTruthy()
    expect(screen.queryByText("name of 10")).toBeNull()
    fireEvent.click(screen.getByText("Show them on this browser"))
    expect(setReveal).toHaveBeenCalledWith(true)
    rerender(
      <PrivacyProvider value={privacyValue(["steam/10"], true, actions, setReveal)}>
        <HiddenGames nameOf={nameOf} />
      </PrivacyProvider>,
    )
    expect(screen.getByText("name of 10")).toBeTruthy()
    rerender(
      <PrivacyProvider value={privacyValue([], false, actions)}>
        <HiddenGames nameOf={nameOf} />
      </PrivacyProvider>,
    )
    expect(screen.getByText("No game is hidden. A game's page hides it.")).toBeTruthy()
  })
})
