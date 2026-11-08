import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Check } from "@/features/status/storage/check"
import { snapshot } from "../../../kit"

const week = 7 * 86_400_000
const verify = (over: Record<string, number>) =>
  snapshot({
    counters: { verify: { running: 0, finished_ms: 0, bytes_done: 0, bytes: 0, problems: 0, every_ms: week, ...over } },
  })

describe("features/status/storage/check", () => {
  it("shows nothing while the check is off", () => {
    const { container } = render(<Check snapshot={snapshot()} />)
    expect(container.textContent).toBe("")
  })
  it("says how far a round is", () => {
    render(<Check snapshot={verify({ running: 1, bytes_done: 2_000_000_000, bytes: 4_000_000_000 })} />)
    expect(screen.getByText("checking · 50 %")).toBeTruthy()
    expect(screen.getByText("Read")).toBeTruthy()
  })
  it("says when the last round finished, when the next is due, and the damage it found", () => {
    render(<Check snapshot={verify({ finished_ms: 1_793_979_522_438, problems: 2 })} />)
    expect(screen.getByText("checked 6 Nov 2026")).toBeTruthy()
    expect(screen.getByText("13 Nov 2026")).toBeTruthy()
    expect(screen.getByText("2 damaged records or files: lanstash's log names them")).toBeTruthy()
  })
  it("says a first round is still to come", () => {
    render(<Check snapshot={verify({})} />)
    expect(screen.getByText("not yet checked")).toBeTruthy()
    expect(screen.getByText("within minutes")).toBeTruthy()
  })
})
