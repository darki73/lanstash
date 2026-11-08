import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { DownloadsTile, DownloadsView, firstByte } from "@/features/status/downloads"
import { keptOf, live, snapshot } from "../../../kit"

const fetching = live(
  snapshot({
    counters: { downloads: { started: 9, completed: 7, failed: 1 } },
    downloads: [
      { key: "a", span: [0, 10], received: 5, total: 20 },
      { key: "a", span: [10, 20], received: 10, total: 20 },
    ],
  }),
)
const day = keptOf({ "downloads.started": [4, 6], "downloads.completed": [3, 5], "downloads.failed": [0, 2] })

describe("features/status/downloads", () => {
  it("shows the downloads done over the range and what is in flight in the tile", () => {
    render(<DownloadsTile live={fetching} kept={day} href="/x" current />)
    expect(screen.getByText("8 done")).toBeTruthy()
    expect(screen.getByText("1 in flight · 2 failed")).toBeTruthy()
  })
  it("lists what is in flight, the range's figures with failures as alarms, and the counters", () => {
    render(<DownloadsView live={fetching} kept={day} />)
    expect(screen.getByRole("heading", { name: "Downloads" })).toBeTruthy()
    expect(screen.getByText("1 object, 2 pieces")).toBeTruthy()
    const failed = screen.getByText("Failed").nextElementSibling
    expect([failed?.textContent, failed?.className.includes("text-danger")]).toEqual(["2", true])
    expect(screen.getByText("Started").nextElementSibling?.textContent).toBe("10")
    expect(screen.getByText("All counters")).toBeTruthy()
  })
  it("leaves the list out while nothing is in flight", () => {
    render(<DownloadsView live={live()} kept={day} />)
    expect(screen.queryByRole("region", { name: "In flight" })).toBeNull()
  })
  it("says the mean time to the origin's first byte over the range, nothing when none was timed", () => {
    const timed = keptOf({ "downloads.answer_wait_ms": [300, 150], "downloads.answers_timed": [2, 1] })
    expect(firstByte(timed)).toBe(150)
    expect(firstByte(keptOf({}))).toBeUndefined()
    render(<DownloadsView live={live()} kept={timed} />)
    expect(screen.getByText("Time to first byte").nextSibling?.textContent).toBe("150 ms")
  })
})
