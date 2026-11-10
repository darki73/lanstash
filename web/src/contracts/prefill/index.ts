/**
 * Prefill: what the API says about the last prefill job (in every status snapshot, and from GET
 * /api/v1/prefill), and the actions that start and stop one. start gives the API's reason when it
 * does not start (such as another prefill running). noPrefill is the actions of a page that
 * cannot prefill.
 */

export type PrefillState = "running" | "done" | "stopped" | "failed"

/** A step of a planning, where the platform names them (Battle.net), in the order they come. */
export type PlanStep = "queued" | "configs" | "indexes" | "manifest"

/** How far a planning has come: files read of all it reads (0: not known yet), and its step. */
export interface Planning {
  readonly done: number
  readonly total: number
  readonly step?: PlanStep | null
}

export interface Prefill {
  readonly game: string
  readonly version: string | null
  /** A pre-release build rather than the current one. */
  readonly prerelease?: boolean
  /** The install it is for, when not the one lanstash is set up for (Battle.net: `OSX,arm64`). */
  readonly install?: string | null
  readonly state: PrefillState
  readonly files_total: number
  readonly files_done: number
  readonly files_skipped: number
  /** The bytes of those the stash already held (not downloaded; absent from older lanstash). */
  readonly bytes_skipped?: number
  readonly files_missing: number
  /** Where the bytes come from: the CDN, lancache's old cache first, or only lancache's old cache. */
  readonly from?: "cdn" | "lancache" | "lancache-only"
  /** Requests taken from lancache, and their bytes. */
  readonly files_local?: number
  readonly bytes_local?: number
  /** Requests lancache held that failed their check (fetched from the CDN instead). */
  readonly files_rejected?: number
  /** Bytes of the build's install, once known. */
  readonly bytes_total?: number
  /** Bytes the job read through the cache (fetched or completed). */
  readonly bytes: number
  readonly current: string | null
  readonly error: string | null
  readonly started_ms: number
  readonly finished_ms: number | null
  /** While the build is being planned: configs and indexes read, of how many (0: not known yet). */
  readonly planning?: Planning | null
  /** The games waiting to be prefilled after this one, in order. */
  readonly queued?: readonly string[]
}

export interface PrefillActions {
  /**
   * Starts prefilling a game (its pre-release build when asked; from lancache's old cache first
   * when asked, or only what lancache holds with "only"; after the running job when queued; for
   * another install than lanstash is set up for, its tags, `OSX,arm64`); undefined when it started
   * or was queued, else why not.
   */
  start(
    platform: string,
    game: string,
    prerelease?: boolean,
    fromLancache?: boolean | "only",
    queue?: boolean,
    install?: string,
  ): Promise<string | undefined>
  /** Takes a game out of a platform's queue; undefined when it went, else why not. */
  unqueue(platform: string, game: string): Promise<string | undefined>
  /** Stops a platform's prefill (and forgets its queue), or every platform's. */
  stop(platform?: string): Promise<void>
}

export const noPrefill: PrefillActions = {
  start: async () => "prefill is not available here",
  unqueue: async () => "prefill is not available here",
  stop: async () => {},
}

/** Whether a prefill is still at work. */
export const running = (p: Prefill | null | undefined): p is Prefill => p?.state === "running"

/** Whether `game` is being prefilled or waits in the queue of job `p`. */
export const waiting = (p: Prefill | null | undefined, game: string): boolean =>
  running(p) && (p.game === game || (p.queued ?? []).includes(game))
