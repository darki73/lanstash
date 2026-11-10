import { fireEvent, render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { Builds } from "@/contracts/builds"
import type { GameDetail } from "@/contracts/game"
import type { PrefillActions } from "@/contracts/prefill"
import { noSession } from "@/contracts/session"
import { AccessProvider } from "@/features/access"
import { fallbackBuild, GameView } from "@/features/platforms/game"
import { snapshot } from "../../../kit"

const usage = [
  { service: "blizzard", game: "osi", name: "Diablo II: Resurrected", objects: 669, bytes: 29_620_000_000 },
]

const build = {
  version: "3.3.7",
  build_config: "623f7a1f73eabb08ccb2b2046e3f9164",
  have: 14_000_000_000,
  want: 29_620_000_000,
  complete: false,
  plan_error: null,
  detail: {
    files_have: 10_000_000,
    files_want: 20_000_000,
    archived_have: 13_990_000_000,
    archived_want: 29_600_000_000,
    file_count: 300,
    part_count: 144_777,
    archive_count: 146,
    requests: 2_000,
    absent: 1,
  },
}

const osi: GameDetail = { game: "osi", product: "osi", region: "eu", build, prerelease: null, error: null }

function actions() {
  const started: string[] = []
  const prefill: PrefillActions = {
    start: async (platform, game, pre, fromLancache) => {
      started.push(`${platform}/${game}${pre ? " pre" : ""}${fromLancache ? " lancache" : ""}`)
      return undefined
    },
    unqueue: async () => undefined,
    stop: async () => {},
  }
  return { started, prefill }
}

const steamUsage = [
  { service: "steam", game: "1422451", name: "Deadlock", group: "1422450", objects: 500, bytes: 9_000 },
  { service: "steam", game: "1422456", name: "Deadlock", group: "1422450", objects: 900, bytes: 1_000 },
]

const steamBuilds = {
  region: "windows",
  lancache: true,
  games: [
    {
      game: "1422450",
      product: null,
      version: "26468569",
      build_config: null,
      have: 10_000,
      want: 15_000,
      plan_error: null,
      current: false,
      error: null,
      lancache_share: { have: 3_000, want: 15_000 },
    },
  ],
}

describe("features/platforms/game", () => {
  it("shows the app's own updates, its depots' included, and no one else's", () => {
    const e = (game: string, at_ms: number) => ({
      at_ms,
      platform: "steam",
      game,
      change: "new-build" as const,
      version: "26468569",
      from: "26400000",
    })
    const updates = { events: [e("1422450", 2), e("99", 1), { ...e("1422450", 0), platform: "blizzard" }] }
    render(
      <GameView
        snapshot={snapshot({ usage: steamUsage })}
        platformId="steam"
        gameId="1422450"
        builds={steamBuilds}
        updates={updates}
      />,
    )
    const panel = screen.getByRole("region", { name: "Updates of Deadlock" })
    expect(within(panel).getAllByRole("listitem")).toHaveLength(1)
    expect(within(panel).getByText("26400000")).toBeTruthy()
  })
  it("shows no updates panel for a game with none", () => {
    render(<GameView snapshot={snapshot({ usage: steamUsage })} platformId="steam" gameId="1422450" />)
    expect(screen.queryByRole("region", { name: "Updates of Deadlock" })).toBeNull()
  })
  it("draws a Steam app's build from the builds view, deletes all its depots at once, and prunes", async () => {
    const { prefill } = actions()
    const removed: string[] = []
    const stash = {
      remove: async (platform: string, game: string) => {
        removed.push(`${platform}/${game}`)
        return undefined
      },
      prunable: async () => "never asked",
      prune: async () => "never asked",
    }
    render(
      <GameView
        snapshot={snapshot({ usage: steamUsage })}
        platformId="steam"
        gameId="1422450"
        builds={steamBuilds}
        prefill={prefill}
        stash={stash}
      />,
    )
    expect(screen.getByRole("listitem", { name: "Deadlock" }).textContent).toContain("version 26468569")
    expect(screen.getByText("lancache's old cache holds about 20 % of it.")).toBeTruthy()
    expect(
      screen.queryByRole("button", { name: /from lancache/ }),
      "lancache holds less of it than the stash: nothing to take",
    ).toBeNull()
    expect(screen.getByRole("region", { name: "Old builds" }), "Steam prunes whole segments of old chunks").toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Delete Deadlock from the stash…" }))
    fireEvent.click(screen.getByRole("button", { name: "Delete Deadlock from the stash" }))
    expect(await screen.findByText("Delete…")).toBeTruthy()
    expect(removed, "one deletion, by the app").toEqual(["steam/1422450"])
  })
  it("draws a library game's build from the catalog", () => {
    const listed = {
      code: "427520",
      name: "Factorio",
      game: "427520",
      version: "26246811",
      known: true,
      download: 1_000,
    }
    expect(fallbackBuild(undefined, listed)).toEqual({
      version: "26246811",
      build_config: "",
      have: 0,
      want: 1_000,
      complete: false,
      plan_error: null,
      detail: null,
    })
    expect(fallbackBuild(undefined, { ...listed, download: undefined })?.want).toBeNull()
    expect(fallbackBuild(undefined, undefined)).toBeNull()
  })

  it("offers a viewer no Prefill", () => {
    const { prefill } = actions()
    render(
      <AccessProvider session={{ name: "player.one", role: "viewer", accounts: true }} actions={noSession}>
        <GameView snapshot={snapshot({ usage })} platformId="blizzard" gameId="osi" detail={osi} prefill={prefill} />
      </AccessProvider>,
    )
    expect(screen.getByRole("heading", { level: 2, name: "Diablo II: Resurrected" })).toBeTruthy()
    expect(screen.queryByRole("button", { name: /^Prefill/ })).toBeNull()
  })
  it("shows a cached game's build: how much is stashed, in what, and a Prefill", () => {
    const { started, prefill } = actions()
    render(
      <GameView
        snapshot={snapshot({ usage })}
        platformId="blizzard"
        gameId="osi"
        detail={osi}
        prefill={prefill}
        art={(p, id, kind) => `/art/${p}/${id}${kind === "icon" ? "/icon" : ""}`}
      />,
    )
    expect(screen.getByRole("heading", { level: 2, name: "Diablo II: Resurrected" })).toBeTruthy()
    expect(screen.getByText("Battle.net · osi")).toBeTruthy()
    const images = [...document.querySelectorAll("img")].map((i) => i.getAttribute("src"))
    expect(images).toContain("/art/blizzard/osi/icon")
    expect(images).toContain("/art/blizzard/osi")
    expect(screen.getByText("region").nextSibling?.textContent).toBe("EU")
    expect(screen.getByText("stored").nextSibling?.textContent).toBe("29.62 GB")
    expect(screen.queryByText("updates"), "not said while it is not known").toBeNull()
    expect(screen.getByText("14.00 GB of 29.62 GB")).toBeTruthy()
    expect(screen.getByText("Inside 146 archives (144,777 files)")).toBeTruthy()
    expect(screen.getByText("Whole files: configs, indexes, loose (300)")).toBeTruthy()
    expect(screen.getAllByText("47 %").length).toBeGreaterThan(0)
    fireEvent.click(screen.getByRole("button", { name: "Prefill Diablo II: Resurrected" }))
    expect(started).toEqual(["blizzard/osi"])
    expect(screen.queryByRole("button", { name: /from lancache/ }), "no lancache cache").toBeNull()
    // The second click of a double click: on a button that waits for the job, not on another start or a Stop.
    const waiting = screen.getByRole("button", { name: "Diablo II: Resurrected is starting" }) as HTMLButtonElement
    expect(waiting.disabled).toBe(true)
    fireEvent.click(waiting)
    expect(started).toEqual(["blizzard/osi"])
  })
  it("lists another platform's install the stash holds some of beside the usual one", () => {
    const installs = {
      platforms: ["Windows", "OSX"],
      architectures: [],
      own: "Windows",
      held: { Windows: { have: 1, want: 2 }, OSX: { have: 13_000_000_000, want: 13_000_000_000 } },
    }
    const { rerender } = render(
      <GameView snapshot={snapshot({ usage })} platformId="blizzard" gameId="osi" detail={{ ...osi, installs }} />,
    )
    expect(screen.getByText("Diablo II: Resurrected · Windows")).toBeTruthy()
    expect(screen.getByText("Diablo II: Resurrected · macOS")).toBeTruthy()
    expect(screen.getByText("13.00 GB of 13.00 GB")).toBeTruthy()
    const none = { ...installs, held: { ...installs.held, OSX: { have: 0, want: 9 } } }
    rerender(
      <GameView
        snapshot={snapshot({ usage })}
        platformId="blizzard"
        gameId="osi"
        detail={{ ...osi, installs: none }}
      />,
    )
    expect(screen.queryByText(/· macOS$/), "nothing of it stashed: no row").toBeNull()
    expect(screen.queryByText("Diablo II: Resurrected · Windows"), "the usual one alone: its plain name").toBeNull()
  })
  it("offers a player to keep a game of its own private, and no one else", async () => {
    const steam = [{ service: "steam", game: "570", name: "Dota 2", objects: 3, bytes: 9 }]
    const catalog = { products: [{ code: "570", name: "Dota 2", game: "570", version: "1", known: true }] }
    const privateGames = { read: async () => [], set: async () => [] }
    const view = (role: "player" | "viewer", permissions: readonly string[], listed = catalog) => (
      <AccessProvider session={{ name: "player.one", role, accounts: true, permissions }} actions={noSession}>
        <GameView
          snapshot={snapshot({ usage: steam })}
          platformId="steam"
          gameId="570"
          catalog={listed}
          privateGames={privateGames}
        />
      </AccessProvider>
    )
    const { rerender } = render(view("player", ["prefill.own"]))
    expect(await screen.findByRole("button", { name: "Keep private" })).toBeTruthy()
    rerender(view("player", ["prefill.own"], { products: [] }))
    expect(screen.queryByRole("button", { name: "Keep private" }), "not its own game").toBeNull()
    rerender(view("viewer", []))
    expect(screen.queryByRole("button", { name: "Keep private" })).toBeNull()
  })
  it("says whether lanstash keeps the game up to date on its own", () => {
    const { rerender } = render(
      <GameView snapshot={snapshot({ usage })} platformId="blizzard" gameId="osi" detail={osi} followed />,
    )
    expect(screen.getByText("updates").nextSibling?.textContent).toBe("automatic")
    rerender(
      <GameView snapshot={snapshot({ usage })} platformId="blizzard" gameId="osi" detail={osi} followed={false} />,
    )
    expect(screen.getByText("updates").nextSibling?.textContent).toBe("by hand")
  })
  it("deletes the game from the stash after asking once more", async () => {
    const removed: string[] = []
    const stash = {
      remove: async (platform: string, game: string) => {
        removed.push(`${platform}/${game}`)
        return removed.length > 1 ? "it is being prefilled" : undefined
      },
    }
    render(<GameView snapshot={snapshot({ usage })} platformId="blizzard" gameId="osi" detail={osi} stash={stash} />)
    expect(screen.getByRole("region", { name: "In the stash" })).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Delete Diablo II: Resurrected from the stash…" }))
    expect(removed, "asks first").toEqual([])
    fireEvent.click(screen.getByRole("button", { name: "Keep" }))
    fireEvent.click(screen.getByRole("button", { name: "Delete Diablo II: Resurrected from the stash…" }))
    expect(screen.getByRole("button", { name: "Delete Diablo II: Resurrected from the stash" }).textContent).toBe(
      "Delete 29.62 GB",
    )
    fireEvent.click(screen.getByRole("button", { name: "Delete Diablo II: Resurrected from the stash" }))
    expect(await screen.findByText("Delete…")).toBeTruthy()
    expect(removed).toEqual(["blizzard/osi"])
    // A refusal says why.
    fireEvent.click(screen.getByRole("button", { name: "Delete Diablo II: Resurrected from the stash…" }))
    fireEvent.click(screen.getByRole("button", { name: "Delete Diablo II: Resurrected from the stash" }))
    expect(await screen.findByText("it is being prefilled")).toBeTruthy()
  })
  it("shows the deletion instead of an empty page, and leads back", () => {
    const deleting = {
      platform: "blizzard",
      game: "sc1live",
      state: "done" as const,
      items_total: 6,
      items_done: 6,
      bytes_total: 5_780_000_000,
      bytes_done: 5_780_000_000,
      failed: 0,
      started_ms: 1_000_000_000,
      finished_ms: 1_000_014_000,
    }
    // sc1live is gone from the stash, and no catalog or detail knows it.
    render(<GameView snapshot={snapshot({ usage, deleting })} platformId="blizzard" gameId="sc1live" />)
    expect(screen.getByRole("region", { name: "Deleted · sc1live" })).toBeTruthy()
    expect(screen.getByRole("link", { name: "Back to Battle.net" }).getAttribute("href")).toBe("/platforms/blizzard")
    expect(screen.queryByText(/Nothing called/)).toBeNull()
  })
  it("gives each product of the directory its own build panel and prefill", () => {
    const { started, prefill } = actions()
    const classic = { ...build, version: "5.5.4", complete: false }
    const wow: GameDetail = {
      ...osi,
      flavours: [
        { product: "wow_classic", name: "World of Warcraft Classic", build: classic, prerelease: null },
        { product: "wowf", name: "WoW: Forever", build: null, prerelease: null },
      ],
    }
    render(
      <GameView snapshot={snapshot({ usage })} platformId="blizzard" gameId="osi" detail={wow} prefill={prefill} />,
    )
    const builds = screen.getByRole("list", { name: "Builds of Diablo II: Resurrected" })
    expect(within(builds).getByRole("listitem", { name: "Diablo II: Resurrected" })).toBeTruthy()
    expect(within(builds).getByRole("listitem", { name: "World of Warcraft Classic" }).textContent).toContain(
      "wow_classic",
    )
    expect(screen.queryByRole("region", { name: /WoW: Forever/ }), "no build known: no panel").toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "Prefill World of Warcraft Classic" }))
    expect(started).toEqual(["blizzard/wow_classic"])
  })
  it("offers lancache's old cache first only where it holds more of the build than the stash", () => {
    const { started, prefill } = actions()
    // The stash holds 14 GB of the build; lancache's old cache, as probed, `have`.
    const holding = (have: number | null): Builds => ({
      region: "eu",
      lancache: true,
      games: [
        {
          game: "osi",
          product: "osi",
          version: "3.3.7",
          build_config: "",
          current: false,
          prerelease: null,
          error: null,
          lancache_share: have === null ? null : { have, want: 29_620_000_000 },
        },
      ],
    })
    const view = (have: number | null, detail: GameDetail = { ...osi, lancache: true }) => (
      <GameView
        snapshot={snapshot({ usage })}
        platformId="blizzard"
        gameId="osi"
        detail={detail}
        builds={holding(have)}
        prefill={prefill}
      />
    )
    const { rerender } = render(view(20_000_000_000))
    fireEvent.click(
      screen.getByRole("button", { name: "Prefill Diablo II: Resurrected from lancache's old cache first" }),
    )
    expect(started).toEqual(["blizzard/osi lancache"])
    for (const [have, why] of [
      [10_000_000_000, "less than the stash holds"],
      [0, "lancache holds none of it"],
      [null, "not probed yet"],
    ] as const) {
      rerender(view(have))
      expect(screen.queryByRole("button", { name: /from lancache/ }), why).toBeNull()
    }
    rerender(view(20_000_000_000, { ...osi, lancache: false }))
    expect(screen.queryByRole("button", { name: /from lancache/ }), "lanstash cannot read it").toBeNull()
    rerender(view(20_000_000_000, { ...osi, lancache: true, build: { ...build, complete: true } }))
    expect(screen.queryByRole("button", { name: /from lancache/ }), "nothing left to take").toBeNull()
  })
  it("shows a game only the catalog knows, with its pre-release build to pre-load", () => {
    const { started, prefill } = actions()
    const sc: GameDetail = {
      game: "sc1live",
      product: "s1",
      region: "eu",
      build: { ...build, have: null, want: null, detail: null },
      prerelease: { ...build, version: "1.24", complete: false },
      error: null,
    }
    const catalog = { products: [{ code: "s1", name: "StarCraft", game: "sc1live", version: "1.23", known: true }] }
    render(
      <GameView
        snapshot={snapshot({ usage })}
        platformId="blizzard"
        gameId="sc1live"
        detail={sc}
        catalog={catalog}
        prefill={prefill}
      />,
    )
    expect(screen.getByRole("heading", { level: 2, name: "StarCraft" })).toBeTruthy()
    expect(screen.getByText("working out what an install downloads…")).toBeTruthy()
    expect(screen.getByText("never")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Pre-load StarCraft 1.24" }))
    expect(started).toEqual(["blizzard/sc1live pre"])
  })
  it("says what working out a build is doing: its step, and how many files are in", () => {
    const planning = (p: GameDetail["build"]): GameDetail => ({
      game: "sc1live",
      product: "s1",
      region: "eu",
      build: p,
      prerelease: null,
      error: null,
    })
    const unplanned = { ...build, have: null, want: null, detail: null }
    const view = (d: GameDetail) => (
      <GameView snapshot={snapshot({ usage })} platformId="blizzard" gameId="sc1live" detail={d} />
    )
    const { rerender } = render(view(planning({ ...unplanned, planning: { done: 312, total: 680, step: "indexes" } })))
    expect(screen.getByText("reading the archives' indexes…")).toBeTruthy()
    expect(screen.getByText("312/680 files read")).toBeTruthy()
    rerender(view(planning({ ...unplanned, planning: { done: 0, total: 0, step: "queued" } })))
    expect(screen.getByText("waiting while another build is worked out…")).toBeTruthy()
    expect(screen.queryByText(/files read/), "no count before the files are known").toBeNull()
    rerender(view(planning({ ...unplanned, planning: null })))
    expect(screen.getByText("working out what an install downloads…"), "no step said: the plain words").toBeTruthy()
  })
  it("offers Stop on the game being prefilled, and says when there is no such game", () => {
    let stops = 0
    const prefill: PrefillActions = {
      ...actions().prefill,
      stop: async () => {
        stops += 1
      },
    }
    const job = {
      game: "osi",
      version: "3.3.7",
      state: "running" as const,
      files_total: 10,
      files_done: 3,
      files_skipped: 0,
      files_missing: 0,
      bytes: 5,
      current: null,
      error: null,
      started_ms: 0,
      finished_ms: null,
    }
    const { unmount } = render(
      <GameView
        snapshot={snapshot({ usage, prefill: job })}
        platformId="blizzard"
        gameId="osi"
        detail={osi}
        prefill={prefill}
      />,
    )
    expect(screen.getByRole("region", { name: "Prefill · Diablo II: Resurrected" })).toBeTruthy()
    // Stopping is asked once more.
    fireEvent.click(screen.getByRole("button", { name: "Stop prefilling Diablo II: Resurrected…" }))
    expect(stops).toBe(0)
    fireEvent.click(screen.getByRole("button", { name: "Keep going" }))
    fireEvent.click(screen.getByRole("button", { name: "Stop prefilling Diablo II: Resurrected…" }))
    fireEvent.click(screen.getByRole("button", { name: "Stop prefilling Diablo II: Resurrected now" }))
    expect(stops).toBe(1)
    unmount()
    // Its pace from the latest snapshots: 16 MB in 2 s is 64 Mbps.
    const at = (now_ms: number, bytes: number) => snapshot({ usage, now_ms, prefill: { ...job, bytes } })
    const paced = render(
      <GameView
        snapshot={at(2000, 16_000_005)}
        history={[at(0, 5), at(1000, 5), at(2000, 16_000_005)]}
        platformId="blizzard"
        gameId="osi"
        detail={osi}
        prefill={prefill}
      />,
    )
    expect(screen.getByText("64.0 Mbps")).toBeTruthy()
    paced.unmount()
    render(<GameView snapshot={snapshot({ usage })} platformId="blizzard" gameId="nothing" />)
    expect(screen.getByText("Nothing called nothing on Battle.net")).toBeTruthy()
  })
})
