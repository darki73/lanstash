/**
 * Platforms model: the game platforms lanstash caches for (lanstash calls them services), each
 * described once and laid out by the same page:
 *   - Battle.net, Steam, Epic Games and Riot Games, always listed in that order, then any other
 *     service the cache holds something of, by name;
 *   - per platform: its mark, a line about it, what its games are called in the store (a product
 *     code, an app's depots), which entries are the launcher's own data rather than games, and a
 *     few facts about how its content arrives.
 * A platform's games are the store's entries grouped into games: on Steam a game arrives as
 * several depots, grouped here by the app they belong to (the API's group; from a lanstash without
 * it, depots that share a name are taken for one game); elsewhere each entry is a game. A named
 * game has art when its platform has an image source: asked for by the app id on Steam (a wide
 * banner and a 32 px icon), by the product path on Battle.net (a portrait key art and a square icon).
 * Steam has an account lanstash signs in to (by QR code, from the platform's page).
 * Each game carries when it was last used, and on a platform whose builds lanstash checks
 * (Battle.net), its current build and whether the stash holds it. The games on an account join
 * these once lanstash can sign in to a platform.
 */
import { type BuildGame, type Builds, buildOf } from "@/contracts/builds"
import type { Prefill } from "@/contracts/prefill"
import type { Download, Snapshot, UsageRow } from "@/contracts/status"
import type { BrandName } from "@/styled/brand"

/** How a platform's game art is asked for and drawn. */
export interface PlatformArt {
  /** What the art is asked for by: a game's group (Steam's app id) or its code (Battle.net's path). */
  readonly by: "group" | "code"
  /** The cover's shape: a wide banner, or a portrait poster cut down to a banner. */
  readonly cover: "wide" | "poster"
  /** Whether there are icons too. */
  readonly icon: boolean
}

/** Whose words describe a platform (its line, its facts): its own, or those of any other service. */
export type Described = "blizzard" | "steam" | "epicgames" | "riot" | "other"

/** A fact about how a platform's content reaches the cache: its key among the words of its platform. */
export type AboutFact =
  | "blizzard.cdn"
  | "blizzard.launcherData"
  | "blizzard.builds"
  | "blizzard.ranges"
  | "blizzard.art"
  | "steam.depots"
  | "steam.crc"
  | "epicgames.directories"
  | "epicgames.sha"
  | "epicgames.oldBuilds"

/** How a platform is described: the parts of its page that differ from platform to platform. */
export interface PlatformKind {
  /** lanstash's service name: the platform's place in links and in the store. */
  readonly id: string
  readonly name: string
  readonly brand: BrandName | undefined
  readonly letters: string
  /** Whose words describe it: its line under the title. */
  readonly described: Described
  /** What the store's id of a game is (a product code, a depot), by its key among the words. */
  readonly code: "productCode" | "depot" | "path" | "id"
  /** What a part of a game is, when a game arrives in several (a depot). */
  readonly part: "depot" | undefined
  /** Store entries that are the launcher's own data, not games; a trailing * matches by prefix. */
  readonly system: readonly string[]
  /** Whether lanstash checks the platform's builds (GET /api/v1/builds/<id>). */
  readonly builds: boolean
  /** What the builds' `region` names: a region of the CDN (Battle.net: eu) or an operating system (Steam: windows). */
  readonly buildsFor: "region" | "os"
  /** Whether lanstash can prefill the platform's games (POST /api/v1/prefill/<id>/<game>). */
  readonly prefill: boolean
  /** Whether lanstash prunes the platform's old builds (/api/v1/games/<id>/<game>/prune). */
  readonly prune: boolean
  /** Whether lanstash signs in to an account of the platform (GET /api/v1/account/<id>). */
  readonly account: boolean
  /** Whether the platform's catalog lists games beyond the cache (GET /api/v1/catalog/<id>). */
  readonly catalog: boolean
  /** Whether a PC needs setting up before lanstash keeps the platform's downloads (Epic Games: its launcher's HTTPS). */
  readonly setup: boolean
  /** How lanstash serves the platform's art, if it does. */
  readonly art: PlatformArt | undefined
  /** Facts about how the platform's content reaches the cache. */
  readonly about: readonly AboutFact[]
}

const kinds: readonly PlatformKind[] = [
  {
    id: "blizzard",
    name: "Battle.net",
    brand: "battlenet",
    letters: "BN",
    described: "blizzard",
    code: "productCode",
    part: undefined,
    system: ["catalogs", "configs", "bnt*", "bdk*"],
    builds: true,
    buildsFor: "region",
    prefill: true,
    prune: true,
    account: false,
    catalog: true,
    setup: false,
    art: { by: "code", cover: "poster", icon: true },
    about: ["blizzard.cdn", "blizzard.launcherData", "blizzard.builds", "blizzard.ranges", "blizzard.art"],
  },
  {
    id: "steam",
    name: "Steam",
    brand: "steam",
    letters: "ST",
    described: "steam",
    code: "depot",
    part: "depot",
    system: [],
    builds: true,
    buildsFor: "os",
    prefill: true,
    prune: true,
    account: true,
    catalog: true,
    setup: false,
    art: { by: "group", cover: "wide", icon: true },
    about: ["steam.depots", "steam.crc"],
  },
  {
    id: "epicgames",
    name: "Epic Games",
    brand: "epicgames",
    letters: "EG",
    described: "epicgames",
    code: "path",
    part: undefined,
    system: [],
    builds: true,
    buildsFor: "os",
    prefill: true,
    prune: true,
    account: true,
    catalog: true,
    setup: true,
    art: { by: "code", cover: "wide", icon: true },
    about: ["epicgames.directories", "epicgames.sha", "epicgames.oldBuilds"],
  },
  {
    id: "riot",
    name: "Riot Games",
    brand: "riotgames",
    letters: "RG",
    described: "riot",
    code: "path",
    part: undefined,
    system: [],
    builds: false,
    buildsFor: "region",
    prefill: false,
    prune: false,
    account: false,
    catalog: false,
    setup: false,
    art: undefined,
    about: [],
  },
]

/** A game of a platform: one store entry, or several parts of one app (or sharing a name). */
export interface Game {
  /** Unique within the platform. */
  readonly key: string
  /** What the API knows the game by (prefill, links): Steam's app id, else its store code. */
  readonly id: string | undefined
  readonly name: string
  /** Whether lanstash knows the game's name (otherwise name is the code). */
  readonly named: boolean
  readonly codes: readonly string[]
  readonly bytes: number
  readonly objects: number
  /** The id to ask for the game's cover art by, when there is art to ask for. */
  readonly art: string | undefined
  /** Last read or write of any of its parts (ms; 0 if unknown). */
  readonly lastUsedMs: number
  /** Its current build, on a platform whose builds lanstash checks. */
  readonly build: BuildGame | undefined
  /** It is kept private: masked (named so, without art, page or builds) or revealed to this browser. */
  readonly privacy?: "masked" | "revealed"
}

export interface Platform extends PlatformKind {
  /** Largest first. */
  readonly games: readonly Game[]
  /** The launcher's own data, largest first. */
  readonly systemData: readonly UsageRow[]
  readonly bytes: number
  readonly objects: number
}

/** Whether `id` names `game`: its id, or one of its store codes. */
export const owns = (game: Game, id: string) => game.id === id || game.codes.includes(id)

/** The link to a platform's page. */
export const platformLink = (id: string) => `/platforms/${id}`

/**
 * The parts of a platform's page: its cached games (the page itself), the games it has beyond the
 * cache (a library or catalog), its prefill job and queue, its account, what is going on, and how to
 * set up a PC for it.
 */
export const sections = ["cached", "library", "prefill", "account", "activity", "setup"] as const
export type Section = (typeof sections)[number]

/** The link to a part of a platform's page. */
export const sectionLink = (id: string, section: Section) =>
  section === "cached" ? platformLink(id) : `${platformLink(id)}/${section}`

/** How much of a build lancache's old cache holds (an estimate), once lanstash probed it. */
export interface LancacheShare {
  readonly have: number
  readonly want: number
}

/**
 * Whether taking a build from lancache's old cache first is worth offering: lanstash can read that
 * cache (`readable`), probed it for the build (`share`), and it holds some of the build and more than
 * the stash does (`stashed` bytes). A build not probed yet, or one lancache holds none of, is not offered.
 */
export function worthLancache(
  readable: boolean | undefined,
  share: LancacheShare | null | undefined,
  stashed: number | null | undefined,
): boolean {
  if (readable !== true || share == null || share.want === 0) return false
  return share.have > 0 && share.have > (stashed ?? 0)
}

/**
 * A platform's last prefill job: its own, from a lanstash that says each platform's; from an older
 * one, the last job of any platform.
 */
export const jobOf = (snapshot: Snapshot, id: string): Prefill | null | undefined =>
  snapshot.prefills === undefined ? snapshot.prefill : (snapshot.prefills[id] ?? null)

/** The link to a game's page: its id on the platform (a Battle.net CDN directory, a Steam app id). */
export const gameLink = (platformId: string, game: string) => `/platforms/${platformId}/${encodeURIComponent(game)}`

const sum = (values: readonly number[]) => values.reduce((a, b) => a + b, 0)
const largestFirst = <T extends { bytes: number }>(a: T, b: T) => b.bytes - a.bytes

/** What a game's art is asked for by: only for a game lanstash can name. */
function artId(kind: PlatformKind, parts: readonly UsageRow[]): string | undefined {
  if (kind.art === undefined || !parts.some((p) => p.name !== null)) return undefined
  if (kind.art.by === "code") return parts[0]?.game
  return parts.find((p) => p.group)?.group ?? undefined
}

/** Whether a store entry is the launcher's own data. */
export function isSystem(kind: PlatformKind, game: string): boolean {
  return kind.system.some((s) => (s.endsWith("*") ? game.startsWith(s.slice(0, -1)) : s === game))
}

function games(kind: PlatformKind, rows: readonly UsageRow[], builds: Builds | undefined): Game[] {
  const groups = new Map<string, UsageRow[]>()
  for (const r of rows) {
    // Entries are grouped by their app where lanstash says it (Steam's depots, Epic's download
    // directories); parts else by the game's name; anything else stays on its own.
    // A masked entry's name is no game's: it never groups by it.
    const key = r.group
      ? `group:${r.group}`
      : kind.part === undefined || r.privacy === "masked"
        ? `code:${r.game}`
        : r.name !== null
          ? `name:${r.name}`
          : `code:${r.game}`
    groups.set(key, [...(groups.get(key) ?? []), r])
  }
  return [...groups]
    .map(([key, parts]) => {
      const sorted = [...parts].sort(largestFirst)
      const first = sorted[0]
      const id = sorted.find((p) => p.group)?.group ?? first?.game
      const privacy: Game["privacy"] = sorted.some((p) => p.privacy === "masked")
        ? "masked"
        : sorted.some((p) => p.privacy === "revealed")
          ? "revealed"
          : undefined
      const masked = privacy === "masked"
      return {
        ...(privacy === undefined ? {} : { privacy }),
        key,
        name: first?.name ?? first?.game ?? key,
        named: first?.name !== null && first?.name !== undefined,
        id,
        codes: sorted.map((p) => p.game),
        bytes: sum(sorted.map((p) => p.bytes)),
        objects: sum(sorted.map((p) => p.objects)),
        // A masked game's art and builds would tell which game it is.
        art: masked ? undefined : artId(kind, sorted),
        lastUsedMs: Math.max(0, ...sorted.map((p) => p.last_used_ms ?? 0)),
        build: kind.builds && id !== undefined && !masked ? buildOf(builds, id) : undefined,
      }
    })
    .sort(largestFirst)
}

function fill(kind: PlatformKind, usage: readonly UsageRow[], builds: Builds | undefined): Platform {
  const rows = usage.filter((u) => u.service === kind.id)
  const system = rows.filter((r) => isSystem(kind, r.game)).sort(largestFirst)
  return {
    ...kind,
    games: games(
      kind,
      rows.filter((r) => !isSystem(kind, r.game)),
      builds,
    ),
    systemData: system,
    bytes: sum(rows.map((r) => r.bytes)),
    objects: sum(rows.map((r) => r.objects)),
  }
}

function other(id: string): PlatformKind {
  return {
    id,
    name: id,
    brand: undefined,
    letters: id.slice(0, 2).toUpperCase(),
    described: "other",
    code: "id",
    part: undefined,
    system: [],
    builds: false,
    buildsFor: "region",
    prefill: false,
    prune: false,
    account: false,
    catalog: false,
    setup: false,
    art: undefined,
    about: [],
  }
}

export function platforms(usage: readonly UsageRow[], builds?: Builds): Platform[] {
  const others = [...new Set(usage.map((u) => u.service))]
    .filter((id) => !kinds.some((k) => k.id === id))
    .sort()
    .map(other)
  return [...kinds, ...others].map((k) => fill(k, usage, builds))
}

/** The platform with an id, if lanstash knows it or holds something of it. */
export function platform(usage: readonly UsageRow[], id: string, builds?: Builds): Platform | undefined {
  return platforms(usage, builds).find((p) => p.id === id)
}

/**
 * A store directory's game as the stash knows it: its id, name and its platform's name; where the
 * stash holds it, what its art is asked for by (with the platform's art) and its page.
 */
export interface DirGame {
  readonly id: string
  readonly name: string
  readonly platform: string
  readonly art?: { readonly id: string; readonly kinds: PlatformArt }
  readonly href?: string
  /** It is the launcher's own data (Battle.net's configs and catalogs), not a game. */
  readonly launcher?: boolean
}

/** What a platform's launcher data is known by among its games: all of it is one entry. */
export const LAUNCHER_DATA = "launcher-data"

/**
 * The game a store directory of a platform belongs to (a Steam depot's app, a Battle.net CDN
 * folder's product), as the stash knows it; the directory itself when it holds none of it. The
 * launcher's own data (Battle.net's configs, catalogs, ...) is all one entry, called `launcher`
 * (the reader's word for it).
 */
export function dirGame(usage: readonly UsageRow[], platformId: string, dir: string, launcher: string): DirGame {
  const p = platform(usage, platformId)
  if (p !== undefined && isSystem(p, dir)) {
    return { id: LAUNCHER_DATA, name: launcher, platform: p.name, launcher: true }
  }
  const g = p?.games.find((x) => owns(x, dir))
  const art = g?.art !== undefined && p?.art !== undefined ? { id: g.art, kinds: p.art } : undefined
  return {
    id: g?.id ?? dir,
    name: g?.named ? g.name : dir,
    platform: p?.name ?? platformId,
    ...(art === undefined ? {} : { art }),
    // A masked game has no page to go to: its address would name it.
    ...(g === undefined || g.privacy === "masked" ? {} : { href: gameLink(platformId, g.id ?? dir) }),
  }
}

/** The downloads in flight for a platform: their keys start with its service name. */
export function downloadsOf(downloads: readonly Download[], id: string): Download[] {
  return downloads.filter((d) => d.key.startsWith(`${id}/`))
}

export type GameOrder = "size" | "name" | "used"

const byOrder: Readonly<Record<GameOrder, (a: Game, b: Game) => number>> = {
  size: largestFirst,
  name: (a, b) => a.name.localeCompare(b.name),
  used: (a, b) => b.lastUsedMs - a.lastUsedMs,
}

/** sortGames orders games by size (largest first), by name, or by last use (latest first). */
export function sortGames(list: readonly Game[], order: GameOrder): Game[] {
  return [...list].sort(byOrder[order])
}
