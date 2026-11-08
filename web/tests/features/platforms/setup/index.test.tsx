import { fireEvent, render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { httpLines, SetupPanel, windowsFile, windowsName } from "@/features/platforms/setup"

describe("features/platforms/setup", () => {
  it("compares the two ways a download goes, then shows Windows: the file to download and who runs what", () => {
    render(<SetupPanel link={(os) => (os === "windows" ? "/api/v1/setup/epicgames/windows" : undefined)} />)
    expect(within(screen.getByRole("listitem", { name: "As Epic comes" })).getByText("nothing kept")).toBeTruthy()
    expect(within(screen.getByRole("listitem", { name: "Set up" })).getByText("kept for every PC")).toBeTruthy()
    const steps = screen.getByRole("list", { name: "Set up Epic Games on Windows" })
    expect(steps.children).toHaveLength(4)
    const download = screen.getByRole("link", { name: `Download ${windowsName}` })
    expect(download.getAttribute("href")).toBe("/api/v1/setup/epicgames/windows")
    expect(download.getAttribute("download")).toBe(windowsName)
    const runs = within(screen.getByRole("list", { name: "What the shortcut runs, and as whom" }))
    expect(runs.getAllByText(/^(user|administrator)$/).map((e) => e.textContent)).toEqual([
      "user",
      "user",
      "administrator",
      "user",
    ])
    expect(runs.getByText(/Only where Windows refuses that/)).toBeTruthy()
    expect(runs.getByText(/never from the administrator's session/)).toBeTruthy()
    expect(screen.getByText(windowsFile)).toBeTruthy()
    expect(httpLines).toContain("ForceNonSslCdn=true")
  })
  it("offers the download disabled where lanstash makes no file", () => {
    render(<SetupPanel />)
    const download = screen.getByRole("button", { name: `Download ${windowsName}` })
    expect((download as HTMLButtonElement).disabled).toBe(true)
  })
  it("switches to macOS and Linux", () => {
    render(<SetupPanel />)
    fireEvent.click(screen.getByRole("button", { name: /macOS/ }))
    expect(screen.getByRole("list", { name: "Set up Epic Games on macOS" })).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: /Linux/ }))
    expect(screen.getByText("Epic has no launcher for Linux")).toBeTruthy()
  })
})
