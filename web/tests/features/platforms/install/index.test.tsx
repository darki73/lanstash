import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Install, installName } from "@/features/platforms/install"

describe("features/platforms/install", () => {
  it("prefills for another platform, with an architecture when there are several", () => {
    const start = vi.fn()
    render(
      <Install
        installs={{ platforms: ["Windows", "OSX"], architectures: ["x86_64", "arm64"] }}
        name="Diablo IV"
        start={start}
      />,
    )
    fireEvent.click(screen.getByRole("button", { name: "macOS" }))
    fireEvent.click(screen.getByRole("button", { name: "Prefill Diablo IV for macOS" }))
    expect(start).toHaveBeenLastCalledWith("OSX")
    fireEvent.change(screen.getByLabelText("Architecture"), { target: { value: "arm64" } })
    fireEvent.click(screen.getByRole("button", { name: "Prefill Diablo IV for macOS" }))
    expect(start).toHaveBeenLastCalledWith("OSX,arm64")
  })
  it("offers no architecture choice for one, and nothing with one platform or none", () => {
    const { rerender, container } = render(
      <Install installs={{ platforms: ["Windows", "OSX"], architectures: ["x86_64"] }} name="X" start={() => {}} />,
    )
    expect(screen.queryByLabelText("Architecture")).toBeNull()
    rerender(<Install installs={{ platforms: ["Windows"], architectures: [] }} name="X" start={() => {}} />)
    expect(container.textContent, "Windows only: nothing else to download for").toBe("")
    rerender(<Install installs={{ platforms: [], architectures: [] }} name="X" start={() => {}} />)
    expect(container.textContent).toBe("")
  })
  it("names an install as people read it", () => {
    const name = (i: string) => installName(i, "en")
    expect([name("OSX,arm64"), name("Windows")]).toEqual(["macOS arm64", "Windows"])
    expect(["windows", "macos", "linux"].map(name)).toEqual(["Windows", "macOS", "Linux"])
    expect([name("OSX,deDE"), name("frFR")]).toEqual(["macOS, German (Germany)", "French (France)"])
  })
  it("prefills another language, with or without another platform", () => {
    const start = vi.fn()
    const { rerender } = render(
      <Install
        installs={{ platforms: ["Windows"], architectures: [], locales: ["enUS", "deDE"] }}
        name="X"
        start={start}
      />,
    )
    expect(screen.queryByRole("button", { name: "Windows" }), "one platform: no choice").toBeNull()
    expect(screen.getByRole("button", { name: "Choose a language" })).toHaveProperty("disabled", true)
    fireEvent.change(screen.getByLabelText("Language"), { target: { value: "deDE" } })
    fireEvent.click(screen.getByRole("button", { name: "Prefill X for German (Germany)" }))
    expect(start).toHaveBeenLastCalledWith("deDE")
    rerender(
      <Install
        installs={{ platforms: ["Windows", "OSX"], architectures: [], locales: ["enUS", "deDE"] }}
        name="X"
        start={start}
      />,
    )
    fireEvent.click(screen.getByRole("button", { name: "macOS" }))
    fireEvent.click(screen.getByRole("button", { name: "Prefill X for macOS, German (Germany)" }))
    expect(start).toHaveBeenLastCalledWith("OSX,deDE")
  })
  it("says which platforms are stashed whole, and offers no prefill of one", () => {
    const start = vi.fn()
    const held = { Windows: { have: 9, want: 9 }, OSX: { have: 9, want: 9 } }
    const { rerender } = render(
      <Install
        installs={{ platforms: ["Windows", "OSX"], architectures: [], own: "Windows", held }}
        name="Hearthstone"
        start={start}
      />,
    )
    expect(screen.getByRole("button", { name: "Windows · stashed" })).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "macOS · stashed" }))
    const done = screen.getByRole("button", { name: "macOS is stashed whole" })
    expect(done).toHaveProperty("disabled", true)
    rerender(
      <Install
        installs={{ platforms: ["Windows", "OSX"], architectures: [], own: "Windows", held: { ...held, OSX: null } }}
        name="Hearthstone"
        start={start}
      />,
    )
    fireEvent.click(screen.getByRole("button", { name: "Prefill Hearthstone for macOS" }))
    expect(start).toHaveBeenLastCalledWith("OSX")
  })
  it("prefills a Steam game for another operating system by Steam's name for it", () => {
    const start = vi.fn()
    render(<Install installs={{ platforms: ["windows", "linux"], architectures: [] }} name="Some Game" start={start} />)
    fireEvent.click(screen.getByRole("button", { name: "Linux" }))
    fireEvent.click(screen.getByRole("button", { name: "Prefill Some Game for Linux" }))
    expect(start).toHaveBeenLastCalledWith("linux")
  })
})
