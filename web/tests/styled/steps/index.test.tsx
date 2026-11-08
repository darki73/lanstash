import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Command, FileCard, Runs, Step, Steps } from "@/styled/steps"

describe("styled/steps", () => {
  it("lists the steps in order, each with its title and what it says", () => {
    render(
      <Steps label="Set up">
        <Step n={1} title="Download">
          <span>the file</span>
        </Step>
        <Step n={2} title="Start" last />
      </Steps>,
    )
    expect(screen.getByRole("list", { name: "Set up" })).toBeTruthy()
    expect(screen.getAllByRole("listitem").map((li) => li.textContent)).toEqual(["1Downloadthe file", "2Start"])
  })
  it("says who runs each part", () => {
    render(
      <Runs
        label="Runs"
        runs={[
          { who: "user", does: "close it" },
          { who: "administrator", does: "write the file" },
        ]}
      />,
    )
    expect(screen.getAllByRole("listitem").map((li) => li.textContent)).toEqual([
      "userclose it",
      "administratorwrite the file",
    ])
  })
  it("offers a file to save, disabled until there is one", () => {
    const { rerender } = render(<FileCard name="a.cmd" note="soon" />)
    expect((screen.getByRole("button", { name: "Download a.cmd" }) as HTMLButtonElement).disabled).toBe(true)
    rerender(<FileCard name="a.cmd" note="ready" href="/files/a" />)
    const link = screen.getByRole("link", { name: "Download a.cmd" })
    expect(link.getAttribute("href")).toBe("/files/a")
    expect(link.getAttribute("download"), "saved under its name").toBe("a.cmd")
  })
  it("copies the lines and says so", async () => {
    const writeText = vi.fn(() => Promise.resolve())
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true })
    render(<Command label="the lines" text={"[A]\nb=1"} />)
    fireEvent.click(screen.getByRole("button", { name: "Copy the lines" }))
    expect(writeText).toHaveBeenCalledWith("[A]\nb=1")
    expect(await screen.findByText("Copied")).toBeTruthy()
  })
})
