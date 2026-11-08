import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { AllCounters, KeptChart, LiveStrip, moving, stateOf } from "@/features/status/monitor"
import { keptOf, live, snapshot } from "../../../kit"

describe("features/status/monitor", () => {
  it("knows a trace is moving by its newest values", () => {
    expect(moving([{ values: [5, ...new Array(20).fill(0)], tone: "cache" }])).toBe(false)
    expect(moving([{ values: [0, 0, 3], tone: "cache" }])).toBe(true)
  })
  it("draws the live strip only while something moves", () => {
    const { rerender } = render(
      <LiveStrip live={live()} label="x" traces={[{ values: [0, 0], tone: "cache" }]} scale="0" />,
    )
    expect(screen.queryByRole("region", { name: "Live" })).toBeNull()
    rerender(<LiveStrip live={live()} label="x" traces={[{ values: [0, 2], tone: "cache" }]} scale="2" />)
    expect(screen.getByRole("region", { name: "Live" })).toBeTruthy()
    expect(screen.getByText("last 2 minutes")).toBeTruthy()
  })
  it("says when what was kept is read or when nothing is", () => {
    const traces = () => []
    const { rerender } = render(<KeptChart kept={undefined} label="x" traces={traces} unit={String} />)
    expect(screen.getByText("Reading what lanstash kept…")).toBeTruthy()
    rerender(<KeptChart kept={null} label="x" traces={traces} unit={String} />)
    expect(screen.getByText("Nothing kept over time")).toBeTruthy()
  })
  it("cuts a burst at the top and says so", () => {
    const values = [...new Array(20).fill(1), 100]
    const k = keptOf({ a: values })
    render(
      <KeptChart kept={k} label="Bytes" traces={(x) => [{ values: x.values("a"), tone: "cache" }]} unit={String} />,
    )
    expect(screen.getByText("25 · peaks to 100 cut")).toBeTruthy()
    expect(screen.getByRole("img", { name: "Bytes per 10 min, last 24 hours" })).toBeTruthy()
  })
  it("folds the counters away under the groups' names", () => {
    render(<AllCounters snapshot={snapshot()} groups={["requests", "https"]} kept={null} />)
    expect(screen.getByText("requests · https")).toBeTruthy()
  })
  it("draws nothing quiet as quiet and trouble as an alarm", () => {
    expect([stateOf(0), stateOf(0, true), stateOf(3), stateOf(3, true)]).toEqual(["quiet", "quiet", undefined, "alarm"])
  })
})
