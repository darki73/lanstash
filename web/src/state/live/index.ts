/**
 * Live status: the latest snapshot from lanstash, the recent ones (for traces over time), and
 * the state of the connection they come on. The feed's functions are what the stream adapter
 * calls; the composition root connects the two, so this knows nothing of event sources. How many
 * snapshots are kept is passed in (two minutes of one a second by default).
 */
import type { Snapshot } from "@/contracts/status"
import { createStore, type Store } from "@/state/store"

/** connecting: no snapshot yet; live: snapshots arrive; reconnecting: the connection dropped. */
export type Connection = "connecting" | "live" | "reconnecting"

export interface LiveStatus {
  readonly connection: Connection
  /** The latest snapshot; kept while reconnecting, so the page does not go blank. */
  readonly snapshot: Snapshot | undefined
  /** The latest snapshots, oldest first, the latest last. */
  readonly recent: readonly Snapshot[]
  /** How many snapshots recent holds at most: the window of a trace. */
  readonly keep: number
}

export interface LiveFeed {
  readonly store: Store<LiveStatus>
  snapshot(s: Snapshot): void
  paused(): void
}

export interface LiveOptions {
  /** How many snapshots recent holds. */
  readonly keep?: number
}

export function createLive(o: LiveOptions = {}): LiveFeed {
  const keep = Math.max(1, o.keep ?? 120)
  const store = createStore<LiveStatus>({ connection: "connecting", snapshot: undefined, recent: [], keep })
  return {
    store,
    snapshot: (snapshot) =>
      store.set((s) => ({ ...s, connection: "live", snapshot, recent: [...s.recent, snapshot].slice(-keep) })),
    paused: () => store.set((s) => (s.connection === "reconnecting" ? s : { ...s, connection: "reconnecting" })),
  }
}
