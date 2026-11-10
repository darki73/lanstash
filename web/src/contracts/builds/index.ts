/**
 * Builds: what GET /api/v1/builds/blizzard says about each Battle.net game in the stash: the
 * product it is, its current build in the region lanstash checks (version is what players see),
 * how many bytes of that build's install the stash holds (have of want; null until lanstash has
 * read the build's download manifest), whether that is all of it (current), a pre-release build
 * the launcher can download ahead of a launch, and why the build is unknown when the lookup
 * failed. buildOf finds a game's entry.
 */

import type { Planning } from "@/contracts/prefill"

/** A build and how much of its install the stash holds. */
export interface BuildRelease {
  readonly version: string
  readonly build_config: string
  readonly have: number | null
  readonly want: number | null
  /** The stash holds all of it. */
  readonly complete: boolean
  /** Why its install could not be worked out. */
  readonly plan_error: string | null
  /** While its install is being worked out (or waits its turn): how far that has come. */
  readonly planning?: Planning | null
}

export interface BuildGame {
  /** The game directory (a CDN path). */
  readonly game: string
  readonly product: string | null
  readonly version: string | null
  readonly build_config: string | null
  readonly have?: number | null
  readonly want?: number | null
  readonly plan_error?: string | null
  readonly planning?: Planning | null
  /** The stash holds all of the current build's install. */
  readonly current: boolean
  readonly prerelease?: BuildRelease | null
  readonly error: string | null
  /** How much of the current build lancache's old cache holds (bytes), once lanstash probed it. */
  readonly lancache_share?: { readonly have: number; readonly want: number } | null
  /** The other products installed from the same directory, with their builds. */
  readonly flavours?: readonly BuildFlavour[]
}

/** Another product of a game's directory (World of Warcraft Classic, ...). */
export interface BuildFlavour {
  readonly product: string
  readonly name: string | null
  readonly build: BuildRelease | null
  readonly prerelease: BuildRelease | null
}

export interface Builds {
  readonly region: string
  /** Whether a prefill can take builds from lancache's old cache first. */
  readonly lancache?: boolean
  readonly games: readonly BuildGame[]
}

/** Each platform's builds, by platform id, once the API said them. */
export type BuildsByPlatform = Readonly<Record<string, Builds>>

export function buildOf(builds: Builds | undefined, game: string): BuildGame | undefined {
  return builds?.games.find((g) => g.game === game)
}
