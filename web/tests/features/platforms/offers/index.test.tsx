import { fireEvent, render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { platform } from "@/features/platforms/model"
import { matching, Offers, plan, sorted, worthImporting } from "@/features/platforms/offers"

const bn = platform([], "blizzard")
const steam = platform([], "steam")
const offers = [
  { code: "s1", name: "StarCraft", game: "sc1live", version: "1.23" },
  { code: "w3", name: "Warcraft III", game: "war3", version: "3.0" },
]
const library = [
  { code: "1", name: "Small", game: "1", version: "1", download: 2e9, lancache: { have: 1e9, want: 2e9 } },
  { code: "2", name: "None", game: "2", version: "1", download: 5e9, lancache: { have: 0, want: 5e9 } },
  { code: "3", name: "Big", game: "3", version: "1", download: 4e9, lancache: { have: 4e9, want: 4e9 } },
]

describe("features/platforms/offers", () => {
  it("lists the games not cached, each leading to its page, and says when there are none", () => {
    if (!bn) throw new Error("no platform")
    const { rerender } = render(<Offers platform={bn} offers={offers} />)
    expect(screen.getByText("2 more games on Battle.net")).toBeTruthy()
    expect(screen.getByRole("link", { name: "StarCraft" }).getAttribute("href")).toBe("/platforms/blizzard/sc1live")
    expect(screen.queryByRole("checkbox"), "nothing to choose without a queue").toBeNull()
    rerender(<Offers platform={bn} offers={[]} />)
    expect(screen.getByText("Every game on Battle.net lanstash knows of is in the cache")).toBeTruthy()
  })
  it("chooses games one by one or all shown, and says what prefilling them takes", () => {
    if (!steam) throw new Error("no platform")
    const queued: [string[], string][] = []
    render(
      <Offers platform={steam} offers={library} free={8e9} lancache queue={(g, from) => queued.push([[...g], from])} />,
    )
    expect(screen.queryByRole("region", { name: "The chosen games" })).toBeNull()
    fireEvent.click(screen.getByRole("checkbox", { name: "Choose all 3 games shown" }))
    const bar = within(screen.getByRole("region", { name: "The chosen games" }))
    expect(
      bar.getByText(
        /3 games chosen · a prefill downloads about 11.00 GB, about 5.00 GB of it from lancache · 8.00 GB free/,
      ),
    ).toBeTruthy()
    expect(bar.getByText("More than is free: eviction will make room.")).toBeTruthy()
    fireEvent.click(bar.getByRole("button", { name: "Queue 3 prefills" }))
    expect(queued).toEqual([[["2", "3", "1"], "prefill"]])
    expect(screen.queryByRole("region", { name: "The chosen games" }), "the choice is cleared").toBeNull()
  })
  it("imports only what lancache holds of the chosen games, most first", () => {
    if (!steam) throw new Error("no platform")
    const queued: [string[], string][] = []
    render(<Offers platform={steam} offers={library} queue={(g, from) => queued.push([[...g], from])} />)
    fireEvent.click(screen.getByRole("checkbox", { name: "Choose Small" }))
    fireEvent.click(screen.getByRole("checkbox", { name: "Choose None" }))
    fireEvent.click(screen.getByRole("checkbox", { name: "Choose Big" }))
    fireEvent.click(screen.getByRole("checkbox", { name: "Choose None" }))
    const all = screen.getByRole("checkbox", { name: "Choose all 3 games shown" }) as HTMLInputElement
    expect(all.checked, "not every game shown is chosen").toBe(false)
    expect(all.indeterminate, "some are").toBe(true)
    fireEvent.click(screen.getByRole("button", { name: "Import what lancache holds of 2 chosen games" }))
    expect(queued).toEqual([[["3", "1"], "lancache-only"]])
  })
  it("shows covers with their art and lancache badge, or the list", () => {
    if (!steam) throw new Error("no platform")
    render(<Offers platform={steam} offers={library} art={(p, id) => `/art/${p}/${id}`} queue={() => {}} />)
    const grid = screen.getByRole("list", { name: "Steam's games to choose from" })
    const big = within(grid).getByRole("listitem", { name: "Big" })
    expect(big.querySelector("img")?.getAttribute("src")).toBe("/art/steam/3")
    expect(within(big).getByText("100 % in lancache")).toBeTruthy()
    expect(within(grid).getByRole("listitem", { name: "None" }).textContent).not.toContain("in lancache")
    fireEvent.click(
      within(screen.getByRole("group", { name: "How to show the games" })).getByRole("button", { name: "List" }),
    )
    expect(screen.queryByRole("list", { name: "Steam's games to choose from" })).toBeNull()
    expect(screen.getByRole("table")).toBeTruthy()
  })
  it("narrows by a search and to the games lancache holds, and sorts them", () => {
    if (!steam) throw new Error("no platform")
    render(<Offers platform={steam} offers={library} queue={() => {}} />)
    fireEvent.click(
      within(screen.getByRole("group", { name: "Which games" })).getByRole("button", { name: "In lancache" }),
    )
    expect(screen.queryByRole("link", { name: "None" })).toBeNull()
    expect(screen.getByRole("link", { name: "Big" })).toBeTruthy()
    const box = screen.getByRole("searchbox", { name: "Search Steam's games" })
    fireEvent.change(box, { target: { value: "zzz" } })
    expect(screen.getByText("No game here matches “zzz”.")).toBeTruthy()
    expect(matching(offers, "sc1live").map((o) => o.name)).toEqual(["StarCraft"])
    expect(sorted(library, "size").map((o) => o.game)).toEqual(["2", "3", "1"])
    expect(sorted(library, "lancache").map((o) => o.game)).toEqual(["3", "1", "2"])
    expect(sorted(library, "name").map((o) => o.name)).toEqual(["Big", "None", "Small"])
    expect(plan([...offers, ...library.slice(0, 1)])).toEqual({ download: 2e9, inLancache: 1e9, unsized: 2 })
    const pubg = {
      code: "578080",
      name: "PUBG",
      game: "578080",
      version: "1",
      download: 54e9,
      lancache: { have: 11.7e6, want: 54e9 },
    }
    expect(worthImporting(pubg), "almost nothing in lancache").toBe(false)
    expect(worthImporting({ ...pubg, lancache: { have: 3e8, want: 54e9 } }), "under 1 %").toBe(false)
    expect(worthImporting({ ...pubg, lancache: { have: 6e8, want: 54e9 } })).toBe(true)
  })
})
