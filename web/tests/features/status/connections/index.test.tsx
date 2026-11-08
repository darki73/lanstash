import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { ConnectionsTile, ConnectionsView } from "@/features/status/connections"
import { keptOf, live, snapshot } from "../../../kit"

const relaying = live(
  snapshot({
    counters: {
      https: { connections: 40, active: 3, relayed: 37 },
      "pass-through": { requests: 5, bytes: 291_371 },
    },
  }),
)
const day = keptOf({
  "https.connections": [20, 24],
  "https.bad_hello": [0, 4],
  "pass-through.bytes": [291_371, 0],
})

describe("features/status/connections", () => {
  it("shows the HTTPS connections over the range and those open now in the tile", () => {
    render(<ConnectionsTile live={relaying} kept={day} href="/x" current={false} />)
    expect(screen.getByText("44 https")).toBeTruthy()
    expect(screen.getByText("3 open now")).toBeTruthy()
  })
  it("lists the range's figures, trouble as alarms, and both counter groups", () => {
    render(<ConnectionsView live={relaying} kept={day} />)
    expect(screen.getByRole("heading", { name: "Connections" })).toBeTruthy()
    expect(screen.getAllByText("291 KB").length).toBeGreaterThanOrEqual(1)
    expect(screen.getByText("4").className).toContain("text-danger")
    expect(screen.getByText("https · pass-through")).toBeTruthy()
  })
})
