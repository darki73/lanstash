import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { keptHitRate, keptSendfile, RequestsTile, RequestsView } from "@/features/status/requests"
import { keptOf, live, snapshot } from "../../../kit"

const at = (now_ms: number, requests: number, hits: number) =>
  snapshot({ now_ms, counters: { requests: { requests, hits, misses: requests - hits } } })
const asked = live(at(0, 0, 0), at(1000, 20, 15))
const day = keptOf({
  "requests.requests": [10, 30],
  "requests.hits": [9, 27],
  "requests.misses": [1, 3],
  "requests.errors": [0, 2],
})

describe("features/status/requests", () => {
  it("shows the hit rate over the range in the tile", () => {
    render(<RequestsTile live={asked} kept={day} href="/x" current={false} />)
    expect(screen.getByText("90.0 % hits")).toBeTruthy()
    expect(screen.getByText("40 requests")).toBeTruthy()
  })
  it("shows the hit rate since start in the tile while nothing over a range is known", () => {
    render(<RequestsTile live={asked} kept={null} href="/x" current={false} />)
    expect(screen.getByText("75.0 % hits")).toBeTruthy()
  })
  it("traces requests and hits a second live, then the range's figures", () => {
    render(<RequestsView live={asked} kept={day} />)
    expect(screen.getByRole("heading", { name: "Requests" })).toBeTruthy()
    expect(screen.getAllByText("20/s").length).toBeGreaterThan(0)
    expect(screen.getByText("15/s")).toBeTruthy()
    expect(screen.getByText("90.0 %")).toBeTruthy()
    expect(screen.getByText("2").className).toContain("text-danger")
  })
  it("works out the hit rate of a range, none for none", () => {
    expect(keptHitRate(day)).toBe(0.9)
    expect(keptHitRate(keptOf({}))).toBeUndefined()
  })
  it("works out the share of body bytes sent from the file, none for none", () => {
    const sent = keptOf({ "requests.sendfile_bytes": [300, 600], "requests.copied_bytes": [100, 0] })
    expect(keptSendfile(sent)).toBe(0.9)
    expect(keptSendfile(keptOf({}))).toBeUndefined()
  })
})
