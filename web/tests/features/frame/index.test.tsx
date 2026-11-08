import { act, render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { HIDDEN_NAME } from "@/contracts/hidden"
import type { Session } from "@/contracts/session"
import type { Version } from "@/contracts/version"
import { Frame } from "@/features/frame"
import { createLive } from "@/state/live"
import { createStore } from "@/state/store"
import { snapshot } from "../../kit"

function frame(path: string, version?: Version) {
  const live = createLive()
  const at = createStore(path)
  const running = createStore<Version | undefined>(version)
  render(<Frame status={live.store} version={running} path={at} />)
  return { live, at, running }
}

const trail = () => screen.getByRole("list", { name: "You are here" }).textContent

describe("features/frame", () => {
  it("waits for the first snapshot, then draws the overview, and keeps it while reconnecting", async () => {
    const { live } = frame("/")
    expect(screen.getByText("connecting…")).toBeTruthy()
    expect(screen.getByText("Waiting for the first snapshot…")).toBeTruthy()
    act(() => live.snapshot(snapshot()))
    expect(screen.getByText("live")).toBeTruthy()
    expect(screen.getByText("up 0h 01m 00s")).toBeTruthy()
    expect(trail()).toBe("Overview")
    expect(
      within(screen.getByRole("navigation", { name: "Main" }))
        .getByRole("link", { name: "Overview" })
        .getAttribute("aria-current"),
    ).toBe("page")
    act(() => live.paused())
    expect(screen.getByText("reconnecting…")).toBeTruthy()
    expect(await screen.findByRole("region", { name: "Status" })).toBeTruthy()
  })
  it("links the trail back to the overview's home from a resource's page", () => {
    const { live } = frame("/overview/storage")
    act(() => live.snapshot(snapshot()))
    expect(
      within(screen.getByRole("list", { name: "You are here" }))
        .getByRole("link", { name: "Overview" })
        .getAttribute("href"),
    ).toBe("/overview")
  })
  it("follows the path to a platform, named in the trail and marked in the sidebar", async () => {
    const { live, at } = frame("/overview/requests")
    act(() => live.snapshot(snapshot()))
    expect(trail()).toBe("Overview/Requests")
    act(() => at.set("/platforms/steam"))
    expect(trail()).toBe("Platforms/Steam")
    expect(screen.getByRole("link", { name: /^Steam/ }).getAttribute("aria-current")).toBe("page")
    expect(await screen.findByRole("heading", { level: 2, name: "Steam" })).toBeTruthy()
    act(() => at.set("/platforms/steam/activity"))
    expect(trail()).toBe("Platforms/Steam/Activity")
    expect(screen.getByRole("link", { name: /^Activity/ }).getAttribute("aria-current")).toBe("page")
    act(() => at.set("/platforms/steam/570"))
    const here = within(screen.getByRole("list", { name: "You are here" }))
    expect(here.getAllByRole("link").map((a) => [a.textContent, a.getAttribute("href")])).toEqual([
      ["Steam", "/platforms/steam"],
    ])
  })
  it("names a hidden game to whoever may hide games, and masks it for anyone else", () => {
    const show = (session: Session) => {
      const live = createLive()
      const hidden = createStore<readonly string[] | null | undefined>(["steam/570"])
      const { unmount } = render(
        <Frame
          status={live.store}
          version={createStore<Version | undefined>(undefined)}
          path={createStore("/platforms/steam/570")}
          session={createStore(session)}
          hidden={hidden}
        />,
      )
      act(() => live.snapshot(snapshot()))
      const said = trail()
      unmount()
      return said
    }
    expect(show({ name: "boss", role: "admin", accounts: true })).toBe("Platforms/Steam/570")
    expect(show({ name: "player.one", role: "player", accounts: true, permissions: ["prefill.own"] })).toBe(
      `Platforms/Steam/${HIDDEN_NAME}`,
    )
  })
  it("shows the version in the footer, and a newer release when there is one", () => {
    const { running } = frame("/", { version: "0.1.0" })
    expect(screen.getByText("lanstash 0.1.0")).toBeTruthy()
    expect(screen.queryByText(/update available/)).toBeNull()
    act(() => running.set({ version: "0.1.0", latest: "0.2.0" }))
    expect(screen.getByText("update available: 0.2.0")).toBeTruthy()
  })
})
