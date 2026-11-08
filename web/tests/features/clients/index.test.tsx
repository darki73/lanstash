import { fireEvent, render, renderHook, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { ArtLink } from "@/contracts/art"
import type { ClientNamer, ClientNow, ClientsReader, ClientTotals } from "@/contracts/clients"
import {
  ClientsView,
  findClients,
  type GameOf,
  gamesOf,
  gotIt,
  momentsOf,
  nowRows,
  shares,
  WhoGotIt,
} from "@/features/clients"
import { useT } from "@/i18n"
import { snapshot } from "../../kit"

/** Depots 71 and 72 are one game, Seven, with art and a page; anything else is its own directory. */
const gameOf: GameOf = (platform, dir) =>
  dir === "71" || dir === "72"
    ? {
        id: "7",
        name: "Seven",
        platform: "Steam",
        art: { id: "7", kinds: { by: "group", cover: "wide", icon: true } },
        href: "/platforms/steam/7",
      }
    : { id: dir, name: dir, platform: platform }

const art: ArtLink = (platform, id, kind = "header") => `/art/${platform}/${id}/${kind}`

const now = (over: Partial<ClientNow>): ClientNow => ({
  address: "10.0.0.5",
  label: null,
  host: "Game-PC",
  service: "steam",
  game: "71",
  per_sec: 1_000_000,
  hit_bytes: 6_000_000,
  miss_bytes: 4_000_000,
  total_bytes: 50_000_000,
  last_ms: 1,
  ...over,
})

const office: ClientTotals = {
  address: "10.0.0.6",
  label: "Office",
  host: "Work-PC",
  hit: 3_000_000_000,
  miss: 1_000_000_000,
  games: [
    { service: "steam", game: "71", hit: 2_000_000_000, miss: 0 },
    { service: "steam", game: "72", hit: 1_000_000_000, miss: 0 },
    { service: "blizzard", game: "tpr", hit: 0, miss: 1_000_000_000 },
  ],
}

describe("features/clients", () => {
  it("adds a game's directories up, now and over a range", () => {
    const rows = nowRows(
      [now({}), now({ game: "72", per_sec: 500_000 }), now({ address: "10.0.0.7", per_sec: 9 })],
      gameOf,
    )
    expect(rows.map((r) => [r.client.address, r.game.name, r.perSec])).toEqual([
      ["10.0.0.5", "Seven", 1_500_000],
      ["10.0.0.7", "Seven", 9],
    ])
    expect(rows[0]?.hit).toBe(12_000_000)
    // An older lanstash says no download of its own: what the client got since it started.
    expect(rows[0]?.session).toBe(100_000_000)
    const sessions = nowRows(
      [now({ session_bytes: 5, since_ms: 9 }), now({ game: "72", session_bytes: 6, since_ms: 4 })],
      gameOf,
    )
    expect([sessions[0]?.session, sessions[0]?.since]).toEqual([11, 4])
    expect(gamesOf(office, gameOf).map((g) => [g.game.name, g.hit + g.miss])).toEqual([
      ["Seven", 3_000_000_000],
      ["tpr", 1_000_000_000],
    ])
  })
  it("draws each download now like a launcher's, with the game's art and a graph", async () => {
    const asked: [number, number][] = []
    const clients: ClientsReader = async (from, to) => {
      asked.push([from, to])
      return [office]
    }
    const recent = [snapshot({ clients: [] }), snapshot({ clients: [now({ per_sec: 2_000_000 })] })]
    render(
      <ClientsView
        now={[now({ session_bytes: 30_000_000, since_ms: 86_430_000 - 300_000 })]}
        recent={recent}
        gameOf={gameOf}
        clients={clients}
        art={art}
        at={86_400_000 + 30_000}
      />,
    )
    const band = screen.getByRole("region", { name: "Game-PC · Steam · Seven" })
    for (const text of ["Seven", "1.0 MB/s", "30.0 MB", "5 min ago"]) {
      expect(within(band).getByText(text)).toBeTruthy()
    }
    expect(band.querySelector("img")?.getAttribute("src")).toBe("/art/steam/7/header")
    expect(within(band).getByText("Game").getAttribute("href")).toBe("/platforms/steam/7")
    expect(momentsOf("10.0.0.5/steam/7", recent, gameOf)).toEqual([{}, { cache: 1_200_000, origin: 800_000 }])
    await screen.findByRole("region", { name: "Office" })
    // The minute that ended 5 s or more ago is the range's end.
    expect(asked).toEqual([[60_000, 86_460_000]])
    expect(screen.getByText("1 downloading now")).toBeTruthy()
  })
  it("gives each client a card with its games, their icons and the cache's share", async () => {
    render(<ClientsView now={[]} gameOf={gameOf} clients={async () => [office]} art={art} at={1} />)
    const card = await screen.findByRole("region", { name: "Office" })
    expect(within(card).getByText("10.0.0.6 · Work-PC")).toBeTruthy()
    expect(within(card).getAllByText("4.00 GB").length).toBeGreaterThan(0)
    expect(within(card).getByText("3.00 GB · 75 %")).toBeTruthy()
    expect(within(card).getByText("Seven")).toBeTruthy()
    expect(within(card).getByText("0 % cache")).toBeTruthy()
    expect([...card.querySelectorAll("img")].map((i) => i.getAttribute("src"))).toEqual(["/art/steam/7/icon"])
  })
  it("renames a client and reads again; a refusal is shown", async () => {
    const named: [string, string][] = []
    let answer: string | undefined = "a name has at most 64 characters"
    const name: ClientNamer = async (address, label) => {
      named.push([address, label])
      return answer
    }
    let reads = 0
    const clients: ClientsReader = async () => {
      reads += 1
      return [office]
    }
    render(<ClientsView now={[]} gameOf={gameOf} clients={clients} name={name} at={1} />)
    const card = await screen.findByRole("region", { name: "Office" })
    fireEvent.click(within(card).getByText("Rename"))
    const input = within(card).getByLabelText("Name of 10.0.0.6")
    fireEvent.change(input, { target: { value: "Study" } })
    fireEvent.click(within(card).getByText("Save"))
    expect(await within(card).findByText("a name has at most 64 characters")).toBeTruthy()
    answer = undefined
    fireEvent.click(within(card).getByText("Save"))
    await screen.findByText("Rename")
    expect(named).toEqual([
      ["10.0.0.6", "Study"],
      ["10.0.0.6", "Study"],
    ])
    expect(reads).toBe(2)
    expect(screen.getByText("nobody right now")).toBeTruthy()
  })
  it("finds clients by name, address or a game they got, and shows the busiest first", async () => {
    const many: ClientTotals[] = Array.from({ length: 14 }, (_, i) => ({
      address: `10.0.1.${i + 1}`,
      label: null,
      host: `PC-${i + 1}`,
      hit: 100 - i,
      miss: 0,
      games: [{ service: "steam", game: i === 3 ? "71" : "99", hit: 1, miss: 0 }],
    }))
    expect(findClients(many, "", gameOf)).toHaveLength(14)
    expect(findClients(many, "pc-12", gameOf).map((c) => c.address)).toEqual(["10.0.1.12"])
    expect(findClients(many, "10.0.1.1", gameOf)).toHaveLength(6)
    expect(findClients(many, "seven", gameOf).map((c) => c.host)).toEqual(["PC-4"])
    const parts = shares(many, renderHook(() => useT("clients")).result.current)
    expect(parts.map((p) => p.label)).toEqual(["PC-1", "PC-2", "PC-3", "PC-4", "PC-5", "9 more"])
    expect(parts.reduce((n, p) => n + p.share, 0)).toBeCloseTo(1)
    render(<ClientsView now={[]} gameOf={gameOf} clients={async () => many} at={1} />)
    await screen.findByRole("region", { name: "PC-1" })
    expect(screen.getByRole("region", { name: "Who got most" })).toBeTruthy()
    expect(screen.queryByRole("region", { name: "PC-13" })).toBeNull()
    fireEvent.click(screen.getByText("Show all 14 clients"))
    expect(screen.getByRole("region", { name: "PC-14" })).toBeTruthy()
    fireEvent.change(screen.getByLabelText("Find a client"), { target: { value: "nothing like it" } })
    expect(screen.getByText("No client matches")).toBeTruthy()
    expect(screen.queryByRole("region", { name: "Who got most" })).toBeNull()
  })
  it("lists who got a game, most first, and nothing when nobody did", async () => {
    const other: ClientTotals = { ...office, address: "10.0.0.8", label: null, host: null, hit: 5, miss: 0, games: [] }
    expect(gotIt([office, other], (s, d) => s === "steam" && d === "72").map((r) => [r.client.address, r.hit])).toEqual(
      [["10.0.0.6", 1_000_000_000]],
    )
    const { container } = render(
      <WhoGotIt clients={async () => [office]} mine={(s, d) => s === "steam" && (d === "71" || d === "72")} at={1} />,
    )
    const panel = await screen.findByRole("region", { name: "Who got it" })
    expect(within(panel).getByText("Office")).toBeTruthy()
    expect(within(panel).getByText("3.00 GB")).toBeTruthy()
    expect(within(panel).getByText("100 % cache")).toBeTruthy()
    render(<WhoGotIt clients={async () => [office]} mine={() => false} at={1} />)
    expect(container.querySelectorAll("section")).toHaveLength(1)
  })
  it("says when nothing is counted or kept", async () => {
    render(<ClientsView now={undefined} gameOf={gameOf} clients={async () => undefined} at={1} />)
    expect(screen.getByText("not counted by this lanstash")).toBeTruthy()
    expect(await screen.findByText("Nothing kept")).toBeTruthy()
  })
})
