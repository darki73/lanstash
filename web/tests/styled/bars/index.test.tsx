import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { BarGroup, BarRow } from "@/styled/bars"

describe("styled/bars", () => {
  it("draws a group with its rows and their bars", () => {
    const { container } = render(
      <BarGroup title="blizzard" value="38.10 GB" detail="1,200 objects">
        <BarRow name="Diablo II: Resurrected" detail="osi" value="29.62 GB" fraction={1} tone="cache" />
        <BarRow name="configs" value="32 KB" fraction={0} tone="cache" />
      </BarGroup>,
    )
    expect(screen.getByText("blizzard")).toBeTruthy()
    expect(screen.getAllByRole("listitem")).toHaveLength(2)
    expect(screen.getByText("osi")).toBeTruthy()
    const bars = [...container.querySelectorAll<HTMLElement>("[data-bar]")].map((b) => b.style.width)
    expect(bars).toEqual(["100%", "0%"])
  })
  it("splits a bar between two tones, with an icon before the name", () => {
    const { container } = render(
      <BarGroup title="Games" value="3 GB">
        <BarRow
          name="Seven"
          value="3 GB"
          fraction={0.5}
          tone="cache"
          split={{ share: 0.75, rest: "origin" }}
          icon={<span>icon</span>}
        />
      </BarGroup>,
    )
    expect(screen.getByText("icon")).toBeTruthy()
    const bar = container.querySelector<HTMLElement>("[data-bar]")
    expect(bar?.style.width).toBe("50%")
    expect([...(bar?.children ?? [])].map((p) => (p as HTMLElement).style.width)).toEqual(["75%", "25%"])
  })
  it("links a row's name to its page, and ends a group with a line under it", () => {
    render(
      <BarGroup title="Games" value="3 GB" more={<span>all of them</span>}>
        <BarRow name="Seven" value="3 GB" fraction={1} tone="cache" href="/platforms/steam/7" />
      </BarGroup>,
    )
    expect(screen.getByRole("link", { name: "Seven" }).getAttribute("href")).toBe("/platforms/steam/7")
    expect(screen.getByText("all of them")).toBeTruthy()
  })
})
