import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { cacheShare, ThroughputTile, ThroughputView } from "@/features/status/throughput"
import { keptOf, live, snapshot } from "../../../kit"

const busy = live(
  snapshot({ rates_mb_s: { "requests.hit_bytes": 100 } }),
  snapshot({ rates_mb_s: { "requests.hit_bytes": 131.2, "requests.miss_bytes": 2, "downloads.fetched_bytes": 3 } }),
)
const day = keptOf({
  "requests.hit_bytes": [3e9, 0, 0],
  "requests.miss_bytes": [1e9, 0, 0],
  "downloads.fetched_bytes": [5e9, 0, 0],
})

describe("features/status/throughput", () => {
  it("shows what clients got over the range and what they get now in the tile", () => {
    render(<ThroughputTile live={busy} kept={day} href="/x" current={false} />)
    expect(screen.getByText("4.00 GB out")).toBeTruthy()
    expect(screen.getByText("now 133.2 MB/s")).toBeTruthy()
  })
  it("shows the rate now in the tile while nothing over a range is known", () => {
    render(<ThroughputTile live={busy} kept={undefined} href="/x" current={false} />)
    expect(screen.getAllByText(/133.2 MB\/s/).length).toBe(2)
  })
  it("draws the live strip while bytes move, then the range and its totals", () => {
    render(<ThroughputView live={busy} kept={day} />)
    expect(screen.getByRole("heading", { name: "Throughput" })).toBeTruthy()
    expect(screen.getByRole("region", { name: "Live" })).toBeTruthy()
    expect(screen.getAllByText("131.2 MB/s").length).toBeGreaterThan(0)
    expect(screen.getByText("Last 24 hours")).toBeTruthy()
    expect(screen.getAllByText("3.00 GB").length).toBeGreaterThan(0)
    expect(screen.getByText("75 %")).toBeTruthy()
  })
  it("leaves the live strip out when nothing moves", () => {
    render(<ThroughputView live={live()} kept={day} />)
    expect(screen.queryByRole("region", { name: "Live" })).toBeNull()
  })
  it("says the cache's share of what clients got, none for nothing", () => {
    expect(cacheShare(day)).toBe(0.75)
    expect(cacheShare(keptOf({}))).toBeUndefined()
  })
})
