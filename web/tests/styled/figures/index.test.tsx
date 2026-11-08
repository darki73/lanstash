import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Figure, Figures } from "@/styled/figures"

describe("styled/figures", () => {
  it("draws each label over its figure", () => {
    const { container } = render(
      <Figures>
        <Figure label="From cache" value="131.2 MB/s" tone="cache" large />
        <Figure label="Downloading" value="0.0 MB/s" tone="plain" dashed />
        <Figure label="Downloaded" value="0 B" />
      </Figures>,
    )
    expect(screen.getByText("131.2 MB/s").tagName).toBe("DD")
    expect(screen.getByText("Downloaded").tagName).toBe("DT")
    expect(container.querySelectorAll("dt > span[aria-hidden]")).toHaveLength(2)
  })
  it("heads the figures when asked, and draws quiet figures faint and alarms in the danger colour", () => {
    render(
      <Figures title="Last 24 hours">
        <Figure label="Failed" value="3" state="alarm" />
        <Figure label="Joined" value="0" state="quiet" />
      </Figures>,
    )
    expect(screen.getByText("Last 24 hours")).toBeTruthy()
    expect(screen.getByText("3").className).toContain("text-danger")
    expect(screen.getByText("0").className).toContain("text-faint")
  })
})
