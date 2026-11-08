import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Download } from "@/styled/download"

describe("styled/download", () => {
  it("draws a download with its art, readings, graph, state and bar", () => {
    const { container } = render(
      <Download
        title="Cyberpunk 2077"
        doing="Prefill"
        image="/art/1091500"
        readings={[{ label: "Network", value: "24.2 Mbps", tone: "origin" }]}
        moments={[{ origin: 1 }, { cache: 1, origin: 2 }]}
        fraction={0.42}
        state="3 minutes left"
        percent="42.0 %"
        action={<button type="button">Stop</button>}
      />,
    )
    const band = screen.getByRole("region", { name: "Prefill · Cyberpunk 2077" })
    expect([...band.querySelectorAll("img")].map((i) => i.getAttribute("src"))).toEqual([
      "/art/1091500",
      "/art/1091500",
    ])
    expect(screen.getByRole("heading", { name: "Cyberpunk 2077" })).toBeTruthy()
    expect(screen.getByText("Network").nextSibling?.textContent).toBe("24.2 Mbps")
    expect(screen.getByText("3 minutes left")).toBeTruthy()
    expect(screen.getByRole("meter", { name: "Cyberpunk 2077: 42.0 %" }).getAttribute("value")).toBe("42")
    // Two moments, the second one stacked of two tones and the tallest.
    const bars = [...container.querySelectorAll("[style]")]
      .map((b) => (b as HTMLElement).style.height)
      .filter(Boolean)
      .map((h) => Math.round(Number.parseFloat(h)))
    expect(bars).toEqual([19, 19, 37])
  })
  it("leaves out the readings and the graph when compact", () => {
    render(
      <Download
        compact
        title="x"
        doing="Prefill"
        readings={[{ label: "Network", value: "1 bps" }]}
        moments={[{ origin: 1 }]}
        fraction={0}
        state="s"
        percent="0 %"
      />,
    )
    expect(screen.queryByText("Network")).toBeNull()
  })
})
