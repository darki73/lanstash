import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Home } from "@/features/status/home"
import { keptOf, live, snapshot } from "../../../kit"

const day = keptOf({ "requests.hit_bytes": [3e9], "requests.miss_bytes": [1e9] })

describe("features/status/home", () => {
  it("shows the savings, the traffic, the games sent most and the stash, without a live strip when idle", () => {
    render(<Home live={live()} kept={day} served={[{ name: "served/steam/10", value: 2e9 }]} />)
    expect(screen.getByRole("region", { name: "What lanstash did" })).toBeTruthy()
    expect(screen.getByRole("region", { name: "Traffic" })).toBeTruthy()
    expect(screen.getByRole("region", { name: "Most served" })).toBeTruthy()
    expect(screen.getByRole("region", { name: "The stash" })).toBeTruthy()
    expect(screen.queryByRole("region", { name: "Live" })).toBeNull()
    expect(screen.queryByRole("region", { name: "Downloading now" })).toBeNull()
  })
  it("shows who downloads what while anyone does", () => {
    const s = snapshot({
      usage: [{ service: "steam", game: "10", name: "Game A", group: "1", objects: 1, bytes: 1 }],
      clients: [
        {
          address: "192.0.2.5",
          label: "Desk",
          host: null,
          service: "steam",
          game: "10",
          per_sec: 20e6,
          hit_bytes: 3,
          miss_bytes: 1,
          total_bytes: 4,
          last_ms: 0,
        },
      ],
    })
    render(<Home live={live(s)} kept={day} served={[]} />)
    expect(screen.getByRole("region", { name: "Downloading now" })).toBeTruthy()
    expect(screen.getByRole("link", { name: "Desk" })).toBeTruthy()
    expect(screen.getAllByText("20.0 MB/s").length).toBeGreaterThan(0)
  })
})
