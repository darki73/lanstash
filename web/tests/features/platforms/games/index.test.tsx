import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Games } from "@/features/platforms/games"
import { platform } from "@/features/platforms/model"

const row = (service: string, game: string, bytes: number, name: string | null) => ({
  service,
  game,
  name,
  objects: 3,
  bytes,
})

const names = () =>
  screen
    .getAllByRole("listitem")
    .flatMap((li) => li.querySelector(":scope > div span.font-medium")?.firstChild?.textContent ?? [])

/** A card's line: its label and value, as "label │ value", and its hint. */
const line = (label: string) => {
  const li = screen.getByText(label).closest("li")
  return `${li?.firstChild?.textContent} │ ${li?.lastChild?.textContent} (${li?.getAttribute("title")})`
}

describe("features/platforms/games", () => {
  it("shows the games as cards, by size, then by name when asked", () => {
    const p = platform(
      [row("blizzard", "hs", 9_000, "Hearthstone"), row("blizzard", "osi", 1_000, "Diablo")],
      "blizzard",
    )
    if (!p) throw new Error("no platform")
    render(<Games now={0} platform={p} />)
    expect(screen.getByText("2 games in the cache")).toBeTruthy()
    expect(names()).toEqual(["Hearthstone", "Diablo"])
    fireEvent.click(screen.getByRole("button", { name: "Name" }))
    expect(screen.getByRole("button", { name: "Name" }).getAttribute("aria-pressed")).toBe("true")
    expect(names()).toEqual(["Diablo", "Hearthstone"])
  })
  it("says a nameless game has no name yet, and counts a game's parts", () => {
    const steam = platform(
      [row("steam", "1", 5, "Deadlock"), row("steam", "2", 5, "Deadlock"), row("steam", "9", 1, null)],
      "steam",
    )
    if (!steam) throw new Error("no platform")
    render(<Games now={0} platform={steam} />)
    expect(screen.getByText("2 depots · 6 objects")).toBeTruthy()
    expect(screen.queryByText("1, 2")).toBeNull()
    expect(screen.getByText("no name yet")).toBeTruthy()
  })
  it("asks the art link for each game with art", () => {
    const steam = platform(
      [{ ...row("steam", "1422451", 5, "Deadlock"), group: "1422450" }, row("steam", "9", 1, null)],
      "steam",
    )
    if (!steam) throw new Error("no platform")
    const art = (p: string, id: string, kind?: string) => `/art/${p}/${id}${kind === undefined ? "" : `/${kind}`}`
    const { container } = render(<Games now={0} platform={steam} art={art} />)
    expect([...container.querySelectorAll("img")].map((i) => i.getAttribute("src"))).toEqual([
      "/art/steam/1422450",
      "/art/steam/1422450/icon",
    ])
  })
  it("gives Battle.net games their key art as a poster and their icon", () => {
    const bn = platform([row("blizzard", "osi", 5, "Diablo II"), row("blizzard", "bnt004", 1, null)], "blizzard")
    if (!bn) throw new Error("no platform")
    const { container } = render(
      <Games now={0} platform={bn} art={(p, id, kind) => `/art/${p}/${id}/${kind ?? "header"}`} />,
    )
    expect([...container.querySelectorAll("img")].map((i) => i.getAttribute("src"))).toEqual([
      "/art/blizzard/osi/header",
      "/art/blizzard/osi/icon",
    ])
    expect(container.querySelector("img")?.className).toContain("object-[center_82%]")
  })
  it("shows each game's build and last use, and sorts by last use", () => {
    const now = 10 * 3_600_000
    const bn = platform(
      [
        { ...row("blizzard", "osi", 9, "Diablo II"), last_used_ms: now - 3 * 3_600_000 },
        { ...row("blizzard", "hs", 1, "Hearthstone"), last_used_ms: now - 60_000 },
      ],
      "blizzard",
      {
        region: "eu",
        games: [
          { game: "osi", product: "osi", version: "3.3", build_config: "a", current: true, error: null },
          {
            game: "hs",
            product: "hsb",
            version: "37.0",
            build_config: "b",
            have: 4_150,
            want: 10_000,
            current: false,
            prerelease: {
              version: "38.0",
              build_config: "c",
              have: null,
              want: null,
              complete: false,
              plan_error: null,
            },
            error: null,
          },
          {
            game: "pro",
            product: "pro",
            version: "2.1",
            build_config: "d",
            have: null,
            want: null,
            current: false,
            error: null,
          },
        ],
      },
    )
    if (!bn) throw new Error("no platform")
    render(<Games now={now} platform={bn} />)
    expect(screen.queryByText("current"), "a current build needs no badge").toBeNull()
    expect(screen.getByText("Version: 3.3")).toBeTruthy()
    expect(screen.queryByText("osi")).toBeNull()
    expect(screen.getByText("41 %").closest("[title]")?.getAttribute("title")).toBe(
      "4 KB of 10 KB of version 37.0 stashed",
    )
    expect(screen.getByText("Version: 37.0")).toBeTruthy()
    expect(line("Pre-release 38.0")).toBe("Pre-release 38.0 │ checking (Pre-release 38.0)")
    expect(screen.getByText("Used 3 h ago")).toBeTruthy()
    expect(names()).toEqual(["Diablo II", "Hearthstone"])
    fireEvent.click(screen.getByRole("button", { name: "Recent" }))
    expect(names()).toEqual(["Hearthstone", "Diablo II"])
  })
  it("offers a prefill per game with a build, Stop on the one running, none on others meanwhile", () => {
    const build = (game: string, current: boolean) => ({
      game,
      product: game,
      version: "1",
      build_config: "a",
      current,
      prerelease:
        game === "hs" ? { version: "2", build_config: "p", have: 1, want: 9, complete: false, plan_error: null } : null,
      error: null,
      have: 2,
      want: 9,
      // lancache's old cache holds more of Hearthstone than the stash, none of Diablo III.
      lancache_share: game === "hs" ? { have: 5, want: 9 } : { have: 0, want: 9 },
    })
    const bn = platform(
      [
        row("blizzard", "osi", 9, "Diablo II"),
        row("blizzard", "hs", 1, "Hearthstone"),
        row("blizzard", "x", 1, "X"),
        row("blizzard", "d3", 1, "Diablo III"),
      ],
      "blizzard",
      { region: "eu", games: [build("osi", true), build("hs", false), build("d3", false)] },
    )
    if (!bn) throw new Error("no platform")
    const started: string[] = []
    const start = (g: string, pre: boolean) => started.push(pre ? `${g} pre` : g)
    const stop = vi.fn()
    const { rerender } = render(<Games now={0} platform={bn} prefill={{ job: null, start, stop }} />)
    expect(screen.queryByRole("button", { name: "Prefill X" })).toBeNull()
    expect(screen.queryByRole("button", { name: /Pre-load Diablo II/ })).toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "Prefill Hearthstone" }))
    fireEvent.click(screen.getByRole("button", { name: "Pre-load Hearthstone 2" }))
    expect(started).toEqual(["hs", "hs pre"])
    expect(screen.queryByRole("button", { name: /from lancache/ }), "no lancache").toBeNull()
    // With lancache's old cache: offered for a build not stashed whole.
    const fromLancache = (g: string, _pre: boolean, lancache?: boolean) => started.push(lancache ? `${g} lancache` : g)
    rerender(<Games now={0} platform={bn} prefill={{ job: null, start: fromLancache, stop, lancache: true }} />)
    expect(screen.queryByRole("button", { name: /Diablo II from lancache/ }), "osi is current").toBeNull()
    expect(screen.queryByRole("button", { name: /Diablo III from lancache/ }), "lancache holds none of it").toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "Prefill Hearthstone from lancache's old cache first" }))
    expect(started.at(-1)).toBe("hs lancache")
    const job = {
      game: "hs",
      version: "1",
      state: "running" as const,
      files_total: 2,
      files_done: 1,
      files_skipped: 0,
      files_missing: 0,
      bytes: 1,
      current: null,
      error: null,
      started_ms: 0,
      finished_ms: null,
    }
    rerender(<Games now={0} platform={bn} prefill={{ job, start, stop }} />)
    fireEvent.click(screen.getByRole("button", { name: "Stop prefilling Hearthstone" }))
    expect(stop).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole("button", { name: "Queue Diablo II after the running prefill" }))
    expect(started.at(-1)).toBe("osi")
    // While it runs, the same choice queues: lancache's old cache first, or the internet only.
    const other = { ...job, game: "osi" }
    rerender(<Games now={0} platform={bn} prefill={{ job: other, start: fromLancache, stop, lancache: true }} />)
    fireEvent.click(
      screen.getByRole("button", {
        name: "Queue Hearthstone after the running prefill, from lancache's old cache first",
      }),
    )
    expect(started.at(-1)).toBe("hs lancache")
    fireEvent.click(screen.getByRole("button", { name: "Queue Hearthstone after the running prefill" }))
    expect(started.at(-1), "Queue alone: the internet only").toBe("hs")
    rerender(<Games now={0} platform={bn} prefill={{ job: { ...job, queued: ["osi"] }, start, stop }} />)
    expect(screen.getByRole("button", { name: "Diablo II is queued" }).hasAttribute("disabled")).toBe(true)
    // A player: only its own games, and no Stop (it may start a prefill, not stop one).
    const own = (g: string) => g === "hs"
    rerender(<Games now={0} platform={bn} prefill={{ job: null, start, may: own }} />)
    expect(screen.getByRole("button", { name: "Prefill Hearthstone" })).toBeTruthy()
    expect(screen.queryByRole("button", { name: "Prefill Diablo III" })).toBeNull()
    rerender(<Games now={0} platform={bn} prefill={{ job, start, may: own }} />)
    expect(screen.queryByRole("button", { name: "Stop prefilling Hearthstone" })).toBeNull()
  })
  it("lists the other products of a game's directory under it", () => {
    const flavour = (name: string, version: string, have: number | null, complete = false) => ({
      product: name,
      name,
      build: { version, build_config: "b", have, want: have === null ? null : 4, complete, plan_error: null },
      prerelease: null,
    })
    const wow = {
      game: "wow",
      product: "wow",
      version: "12.1.5",
      build_config: "a",
      current: true,
      error: null,
      flavours: [flavour("Classic", "5.5.4", 1), flavour("Forever", "1.60.1", 4, true), flavour("Era", "1.15.9", null)],
    }
    const bn = platform([row("blizzard", "wow", 9, "World of Warcraft")], "blizzard", { region: "eu", games: [wow] })
    if (!bn) throw new Error("no platform")
    render(<Games now={0} platform={bn} />)
    expect(line("Classic")).toBe("Classic │ 25 % (Classic 5.5.4)")
    expect(line("Forever")).toBe("Forever │ current (Forever 1.60.1)")
    expect(line("Era")).toBe("Era │ checking (Era 1.15.9)")
  })
  it("says how much of a build lancache's old cache holds, until it is stashed", () => {
    const game = (name: string, current: boolean, share: { have: number; want: number } | null) => ({
      game: name,
      product: name,
      version: "1",
      build_config: "a",
      current,
      error: null,
      lancache_share: share,
    })
    const bn = platform(
      [row("blizzard", "d3", 9, "Diablo III"), row("blizzard", "ow", 8, "Overwatch"), row("blizzard", "hs", 7, "HS")],
      "blizzard",
      {
        region: "eu",
        games: [
          game("d3", false, { have: 8, want: 100 }),
          game("ow", true, { have: 98, want: 100 }),
          game("hs", false, null),
        ],
      },
    )
    if (!bn) throw new Error("no platform")
    render(<Games now={0} platform={bn} />)
    expect(line("In lancache")).toBe("In lancache │ 8 % (In lancache)")
    expect(screen.queryAllByText(/In lancache/).length, "not when stashed whole, nor before the probe").toBe(1)
  })
  it("finds games as one types, by name, initials or code, and shows only those behind when asked", () => {
    const build = (game: string, current: boolean) => ({
      game,
      product: game,
      version: "1",
      build_config: "a",
      current,
      error: null,
    })
    const bn = platform(
      [
        row("blizzard", "osi", 9, "Diablo II: Resurrected"),
        row("blizzard", "hs", 8, "Hearthstone"),
        row("blizzard", "wow", 7, "World of Warcraft"),
        row("blizzard", "w3", 6, "Warcraft III"),
        row("blizzard", "sc2", 5, "StarCraft II"),
        row("blizzard", "d3", 4, "Diablo III"),
        row("blizzard", "pro", 3, "Overwatch"),
      ],
      "blizzard",
      { region: "eu", games: [build("osi", false), build("hs", true), build("d3", false)] },
    )
    if (!bn) throw new Error("no platform")
    render(<Games now={0} platform={bn} />)
    const find = screen.getByRole("searchbox", { name: "Find a Battle.net game" })
    fireEvent.change(find, { target: { value: "diablo" } })
    expect(names()).toEqual(["Diablo II: Resurrected", "Diablo III"])
    expect(screen.getByText("2 of 7 games")).toBeTruthy()
    fireEvent.change(find, { target: { value: "sc2" } })
    expect(names(), "by its code").toEqual(["StarCraft II"])
    fireEvent.change(find, { target: { value: "dir" } })
    expect(names(), "by initials").toEqual(["Diablo II: Resurrected"])
    fireEvent.change(find, { target: { value: "zelda" } })
    expect(screen.getByText("No game matches “zelda”")).toBeTruthy()
    fireEvent.keyDown(find, { key: "Escape" })
    expect(names()).toHaveLength(7)
    fireEvent.click(screen.getByRole("button", { name: "2 behind" }))
    expect(names()).toEqual(["Diablo II: Resurrected", "Diablo III"])
    fireEvent.click(screen.getByRole("button", { name: "All" }))
    expect(screen.getByText("7 games in the cache")).toBeTruthy()
  })
  it("queues prefills of all the games behind, the smallest gap first, and says how much they take", () => {
    const build = (game: string, have: number | null, want: number | null) => ({
      game,
      product: game,
      version: "1",
      build_config: "a",
      current: false,
      have,
      want,
      error: null,
    })
    const bn = platform(
      [row("blizzard", "osi", 9, "Diablo II"), row("blizzard", "hs", 8, "Hearthstone"), row("blizzard", "d3", 7, "D3")],
      "blizzard",
      {
        region: "eu",
        games: [build("osi", 1_000, 9_000_000_000), build("hs", 1_000, 2_000_001_000), build("d3", null, null)],
      },
    )
    if (!bn) throw new Error("no platform")
    const queued: string[][] = []
    const pf = { job: null, start: () => {}, stop: () => {}, queueAll: (g: readonly string[]) => queued.push([...g]) }
    render(<Games now={0} platform={bn} prefill={pf} />)
    expect(
      screen.queryByRole("region", { name: "Bring the games behind up to date" }),
      "only with the behind shown",
    ).toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "3 behind" }))
    const bar = screen.getByRole("region", { name: "Bring the games behind up to date" })
    expect(bar.textContent).toContain("3 games behind · about 11.00 GB to download, 1 not worked out yet")
    fireEvent.click(screen.getByRole("button", { name: "Queue prefills of the 3 games behind" }))
    expect(queued, "the smallest gap first; one not worked out yet counts as none").toEqual([["d3", "hs", "osi"]])
  })
  it("folds the launcher data away", () => {
    const p = platform(
      [row("blizzard", "catalogs", 287_000, null), row("blizzard", "configs", 32_000, null)],
      "blizzard",
    )
    if (!p) throw new Error("no platform")
    render(<Games now={0} platform={p} />)
    expect(screen.getByText("catalogs · configs — 319 KB")).toBeTruthy()
    expect(screen.getByText("Nothing from Battle.net in the cache yet")).toBeTruthy()
  })
})
