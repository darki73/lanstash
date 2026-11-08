import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Chart, capped, points } from "@/styled/chart"

describe("styled/chart", () => {
  it("puts the newest value at the right edge and 0 just above the bottom", () => {
    const p = points([0, 10], 3, 10, 100)
    expect(p.map(([x]) => x)).toEqual([500, 1000])
    expect(p[1]?.[1]).toBeCloseTo(0)
    expect(p[0]?.[1]).toBeCloseTo(97)
  })
  it("keeps only the window's worth of values", () => {
    expect(points([1, 2, 3, 4], 2, 4, 100)).toHaveLength(2)
  })
  it("draws a large chart with its scale and window, an area and a line", () => {
    const { container } = render(
      <Chart
        label="MB/s"
        window={10}
        scale="132.0 MB/s"
        span="2 minutes"
        traces={[
          { values: [1, 2, 3], tone: "cache" },
          { values: [3, 2, 1], tone: "origin", style: "dashed" },
        ]}
      />,
    )
    expect(screen.getByRole("img", { name: "MB/s" })).toBeTruthy()
    expect(screen.getByText("132.0 MB/s")).toBeTruthy()
    expect(screen.getByText("2 minutes ago")).toBeTruthy()
    expect(container.querySelectorAll("path")).toHaveLength(3)
    expect(container.querySelector("path[stroke-dasharray]")).toBeTruthy()
  })
  it("caps a burst over the rest, but never under a quarter of the highest", () => {
    expect(capped([...new Array(20).fill(1), 100])).toBe(25)
    expect(capped([...new Array(20).fill(10), 100])).toBe(30)
    expect(capped([...new Array(20).fill(10), 50])).toBeUndefined()
    expect(capped([1, 100])).toBeUndefined()
  })
  it("draws values over a given top cut at it, each with a mark", () => {
    const { container } = render(
      <Chart label="x" window={4} top={10} traces={[{ values: [1, 50, 2, 60], tone: "cache" }]} />,
    )
    expect(container.querySelectorAll("[data-cut]")).toHaveLength(2)
    expect(points([50], 1, 10, 100)[0]?.[1]).toBeCloseTo(0)
  })
  it("draws a small chart without labels, and nothing for an empty trace", () => {
    const { container } = render(<Chart size="small" label="x" window={10} traces={[{ values: [], tone: "plain" }]} />)
    expect(container.querySelectorAll("path")).toHaveLength(0)
    expect(screen.queryByText("now")).toBeNull()
  })
})
