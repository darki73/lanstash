import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { MostServed, mostServed } from "@/features/status/traffic"

const gameOf = (platform: string, game: string) =>
  game.startsWith("56399")
    ? { id: "563560", name: "Alien Swarm", platform: "Steam", href: "/platforms/steam/563560" }
    : { id: game, name: game, platform }

describe("features/status/traffic", () => {
  it("adds up a game's directories before ranking (a Steam app's depots are one game), skipping other counters", () => {
    const got = mostServed(
      [
        { name: "served/steam/563991", value: 2_560 },
        { name: "served/blizzard/wow", value: 920 },
        { name: "served/steam/563992", value: 857 },
        { name: "requests.hits", value: 5 },
      ],
      gameOf,
    )
    expect(got.map((g) => [g.game.name, g.value])).toEqual([
      ["Alien Swarm", 3_417],
      ["wow", 920],
    ])
  })
  it("lists the games sent most over the range, linking to their pages", () => {
    render(
      <MostServed
        counters={[
          { name: "served/steam/563991", value: 4_000_000_000 },
          { name: "served/blizzard/osi", value: 1_000_000_000 },
        ]}
        gameOf={gameOf}
        words="24 hours"
      />,
    )
    expect(screen.getByRole("link", { name: "Alien Swarm" }).getAttribute("href")).toBe("/platforms/steam/563560")
    expect(screen.getByText("osi")).toBeTruthy()
    expect(screen.getByText("last 24 hours")).toBeTruthy()
  })
  it("says when nothing was served", () => {
    render(<MostServed counters={[]} gameOf={gameOf} words="7 days" />)
    expect(screen.getByText("Nothing served")).toBeTruthy()
  })
})
