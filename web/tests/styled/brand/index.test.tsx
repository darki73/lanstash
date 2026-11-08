import { render } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Brand, type BrandName, brandTile } from "@/styled/brand"

describe("styled/brand", () => {
  it("gives every platform a tile colour that shows on both themes", () => {
    const tiles = (["battlenet", "steam", "epicgames", "riotgames"] as const).map((n: BrandName) => brandTile(n))
    expect(tiles).toEqual(["#4381C3", "#171A21", "#2A2A2A", "#EB0029"])
  })
  it("draws the mark in white on its tile, hidden from assistive technology", () => {
    const { container } = render(<Brand name="riotgames" size="large" />)
    const tile = container.firstElementChild as HTMLElement
    expect(tile.getAttribute("aria-hidden")).toBe("true")
    expect(tile.style.backgroundColor).not.toBe("")
    expect(container.querySelector("svg")?.getAttribute("fill")).toBe("#FFFFFF")
    expect(container.querySelector("path")?.getAttribute("d")?.length).toBeGreaterThan(20)
  })
})
