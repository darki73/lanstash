import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { PlatformsNav } from "@/features/platforms/nav"
import { snapshot } from "../../../kit"

const usage = [
  { service: "blizzard", game: "osi", name: "Diablo II: Resurrected", objects: 669, bytes: 29_620_000_000 },
  { service: "blizzard", game: "catalogs", name: null, objects: 8, bytes: 287_000 },
  { service: "steam", game: "1422456", name: "Deadlock", objects: 17_766, bytes: 6_120_000_000 },
]

describe("features/platforms/nav", () => {
  it("links to every platform in the sidebar, with what the cache holds of it", () => {
    render(<PlatformsNav snapshot={snapshot({ usage })} current="blizzard" />)
    const links = screen.getAllByRole("link")
    expect(links.map((a) => a.textContent)).toEqual([
      "Battle.net29.62 GB",
      "Steam6.12 GB",
      "Epic Games–",
      "Riot Games–",
    ])
    expect(links[0]?.getAttribute("href")).toBe("/platforms/blizzard")
    expect(links[0]?.getAttribute("aria-current")).toBe("page")
  })
})
