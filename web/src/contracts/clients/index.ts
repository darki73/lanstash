/**
 * Clients: who downloads what through the cache. Every status snapshot carries the clients
 * downloading now (ClientNow: a client, a store directory of a game, how fast over the last
 * seconds, and how much of that came from the cache); GET /api/v1/clients?from=&to= says what each
 * client got over a range, per store directory, from the counters lanstash keeps by the minute
 * (ClientTotals). "hit" bytes came from the cache, "miss" bytes were sent while lanstash
 * downloaded them. A client is called by the name the user gave it (label), else what the network
 * calls its address (host), else its address; ClientsReader asks for a range, ClientNamer gives a
 * client a name (an empty one takes it away).
 */

/** What a client is called: its address, the name given it, what the network calls it. */
export interface ClientId {
  readonly address: string
  readonly label: string | null
  readonly host: string | null
}

/** A client downloading a game now. */
export interface ClientNow extends ClientId {
  readonly service: string
  /** The store directory (a Steam depot, a Battle.net CDN folder). */
  readonly game: string
  /** Bytes per second over the last seconds. */
  readonly per_sec: number
  /** Of the last seconds' bytes: from the cache, and while downloading. */
  readonly hit_bytes: number
  readonly miss_bytes: number
  /** All it got of the directory since lanstash started. */
  readonly total_bytes: number
  /**
   * What it got in this download, and when that started: a client that got nothing of a game for
   * two minutes starts a new one (absent from an older lanstash).
   */
  readonly session_bytes?: number
  readonly since_ms?: number
  readonly last_ms: number
}

/** What a client got of one store directory over a range. */
export interface GameBytes {
  readonly service: string
  readonly game: string
  readonly hit: number
  readonly miss: number
}

/** What a client got over a range, the directory it got most first. */
export interface ClientTotals extends ClientId {
  readonly hit: number
  readonly miss: number
  readonly games: readonly GameBytes[]
}

/** Asks what each client got from from_ms to to_ms; undefined when lanstash could not say. */
export type ClientsReader = (from_ms: number, to_ms: number) => Promise<readonly ClientTotals[] | undefined>

export const noClients: ClientsReader = async () => undefined

/** Names the client at `address` (an empty label takes its name away); why not, when it failed. */
export type ClientNamer = (address: string, label: string) => Promise<string | undefined>

/** The name a client is shown by. */
export const clientName = (c: ClientId): string => c.label ?? c.host ?? c.address

/** The share (0 to 1) of `hit` in `hit + miss`; undefined when both are 0. */
export const cacheShare = (hit: number, miss: number): number | undefined =>
  hit + miss > 0 ? hit / (hit + miss) : undefined
