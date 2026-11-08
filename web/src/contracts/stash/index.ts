/**
 * Stash: what the app can do to what lanstash keeps of a game: delete it (all of it, as a job whose
 * progress every status snapshot carries; it comes back
 * when a client downloads the game again). remove gives undefined when it went, else why not.
 * noStash is the actions of a page that cannot change the stash.
 */

export interface StashActions {
  /** Starts deleting what the stash holds of a game (one of its codes on the platform). */
  remove(platform: string, game: string): Promise<string | undefined>
  /** What a prune of a game would delete (old builds' files), or why that is unknown. */
  prunable?(platform: string, game: string): Promise<Prunable | string>
  /** Starts pruning a game: deleting what no current build needs; undefined when it started, else why not. */
  prune?(platform: string, game: string): Promise<string | undefined>
}

/** What a prune would delete: object files no current build needs, unused for the grace period. */
export interface Prunable {
  readonly files: number
  readonly bytes: number
  /** Bytes no current build needs but used lately, kept for now. */
  readonly recent_bytes: number
  /** Of `files`, the segments compacted: the chunks current builds need are copied out first (Steam). */
  readonly compact?: number
}

/** The last deletion of a game (in every status snapshot), running or done. */
export interface Deleting {
  readonly platform: string
  readonly game: string
  readonly state: "running" | "done"
  /** Segments and object files to delete, and deleted. */
  readonly items_total: number
  readonly items_done: number
  readonly bytes_total: number
  readonly bytes_done: number
  /** How many could not be deleted. */
  readonly failed: number
  /** Only old builds' files went (a prune), not the whole game. */
  readonly partial?: boolean
  readonly started_ms: number
  readonly finished_ms: number | null
}

export const noStash: StashActions = {
  remove: async () => "deleting is not available here",
}

/** How long a finished deletion is still shown. */
export const deletedShownMs = 10 * 60_000

/** The deletion worth showing at `now`: one running, or one finished within {@link deletedShownMs}. */
export const recentDeletion = (d: Deleting | null | undefined, now: number): Deleting | undefined =>
  d != null && (d.state === "running" || now - (d.finished_ms ?? now) < deletedShownMs) ? d : undefined
