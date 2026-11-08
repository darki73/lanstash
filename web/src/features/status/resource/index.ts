/**
 * Resource: what every resource of the monitor (throughput, downloads, requests, storage,
 * connections) is given: the live data, what lanstash kept over the overview's range, and for its
 * tile the link that shows it and whether it is shown. A resource's name, as its tile and the trail
 * say it, is the key `resource.<id>` of the status words.
 */
import type { ArtLink } from "@/contracts/art"
import type { Snapshot } from "@/contracts/status"
import type { Kept } from "@/features/status/kept"

/** The data a resource draws: the latest snapshot, the recent ones, the window of a trace. */
export interface Live {
  readonly snapshot: Snapshot
  readonly recent: readonly Snapshot[]
  readonly keep: number
}

export interface TileArgs {
  readonly live: Live
  readonly href: string
  readonly current: boolean
  /** What lanstash kept over the overview's range: undefined while read, null when none is kept. */
  readonly kept: Kept | null | undefined
}

/** What a resource's page is given: the live data and what was kept over the range. */
export interface ViewArgs {
  readonly live: Live
  readonly kept: Kept | null | undefined
  /** Where a game's icon is; letters stand in without it. */
  readonly art?: ArtLink
}

export const resources = ["throughput", "downloads", "requests", "storage", "connections"] as const
export type ResourceId = (typeof resources)[number]

/** The link to a resource on the overview. */
export const overviewLink = (resource: ResourceId) => `/overview/${resource}`

/** lanstash sends one snapshot a second: a window of keep snapshots is keep seconds. */
export const secondsPerSnapshot = 1
