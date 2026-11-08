/**
 * Game: one Battle.net game, from GET /api/v1/games/blizzard/<game>: its product, the region
 * lanstash checks, its current build and its pre-release build (if any), each with how much of its
 * install the stash holds, and, once lanstash read the build's download manifest, that split into
 * whole files (configs, indexes, loose files) and files inside archives.
 */
import type { BuildRelease } from "@/contracts/builds"

/** A build's install in figures. */
export interface PlanFigures {
  readonly files_have: number
  readonly files_want: number
  readonly archived_have: number
  readonly archived_want: number
  /** Whole files. */
  readonly file_count: number
  /** Files inside archives. */
  readonly part_count: number
  /** The archives they are in. */
  readonly archive_count: number
  /** Requests a prefill sends. */
  readonly requests: number
  /** Files the CDN does not have. */
  readonly absent: number
}

export interface GameBuild extends BuildRelease {
  readonly detail: PlanFigures | null
}

export interface GameDetail {
  readonly game: string
  readonly product: string | null
  readonly region: string
  readonly build: GameBuild | null
  readonly prerelease: GameBuild | null
  readonly error: string | null
  /** Whether a prefill can take what lancache's old cache holds of it first. */
  readonly lancache?: boolean
  /** The other products installed from the same directory (World of Warcraft Classic, ...). */
  readonly flavours?: readonly GameFlavour[]
  /**
   * The platforms, architectures and locales the current build has installs for (its download
   * manifest's tags: `Windows`, `OSX`; `x86_64`, `arm64`; `enUS`, `deDE`), so a prefill may be for
   * another platform's or language's install; null until its plan is made, absent from an older
   * lanstash.
   */
  readonly installs?: GameInstalls | null
}

/** What a game has installs for, and what the stash holds of each platform's. */
export interface GameInstalls {
  readonly platforms: readonly string[]
  readonly architectures: readonly string[]
  readonly locales?: readonly string[]
  /** The platform lanstash is set up for, when it is one of them. */
  readonly own?: string | null
  /** Bytes of each platform's install the stash holds; null while lanstash works it out. */
  readonly held?: Readonly<Record<string, { readonly have: number; readonly want: number } | null>>
}

/** Another product of a game's directory: its code (what a prefill of it asks for), name and builds. */
export interface GameFlavour {
  readonly product: string
  readonly name: string | null
  readonly build: GameBuild | null
  readonly prerelease: GameBuild | null
}
