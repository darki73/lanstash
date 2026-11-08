import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Tile, TileRow } from "@/styled/tile"

describe("styled/tile", () => {
  it("links to its resource, and says when it is the one shown", () => {
    render(
      <>
        <Tile name="Storage" value="41.27 GB" detail="3.48 TB free" tone="cache" href="/overview/storage" current>
          chart
        </Tile>
        <Tile
          name="Requests"
          value="– hits"
          detail="0 requests"
          tone="cache"
          href="/overview/requests"
          current={false}
        />
      </>,
    )
    const [storage, requests] = screen.getAllByRole("link")
    expect([storage?.getAttribute("href"), storage?.getAttribute("aria-current")]).toEqual([
      "/overview/storage",
      "page",
    ])
    expect(requests?.getAttribute("aria-current")).toBeNull()
    expect(screen.getByText("3.48 TB free")).toBeTruthy()
    expect(screen.getByText("chart")).toBeTruthy()
  })
  it("lays tiles out in a row named for assistive technology", () => {
    render(
      <TileRow label="Resources">
        <Tile name="A" value="1" detail="d" tone="cache" href="/a" current={false} />
      </TileRow>,
    )
    expect(screen.getByRole("navigation", { name: "Resources" }).querySelectorAll("a")).toHaveLength(1)
  })
})
