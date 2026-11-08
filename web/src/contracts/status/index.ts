/**
 * Status: the snapshot lanstash's API sends, one per interval on /api/v1/events and once on
 * /api/v1/status. Byte counts are bytes, times are Unix milliseconds,
 * rates are MB/s (10^6 bytes) over the time since the previous snapshot of the same viewer.
 */
import type { ClientNow } from "@/contracts/clients"
import type { Prefill } from "@/contracts/prefill"
import type { Deleting } from "@/contracts/stash"

/** One download in flight: a whole object, or one piece (byte range) of it. */
export interface Download {
  /** The object: service, game and path. */
  readonly key: string
  /** The range fetched, [start, end); end is null when unknown; the whole object is null. */
  readonly span: readonly [number, number | null] | null
  /** The position reached: from the start of the object, not of the range. */
  readonly received: number
  /** The object's size, when the origin said it. */
  readonly total: number | null
}

/** Stored objects and bytes of one game. */
export interface UsageRow {
  readonly service: string
  readonly game: string
  /** Display name, when a names source knows the game. */
  readonly name: string | null
  /** The game the entry belongs to (a Steam depot's app id), when known; absent from older lanstash. */
  readonly group?: string | null
  readonly objects: number
  readonly bytes: number
  /** Last read or write of the entry (ms since the epoch; 0 never); absent from older lanstash. */
  readonly last_used_ms?: number
  /**
   * Not from lanstash: the page marks the entry of a game kept private (contracts/hidden),
   * masked (its name already replaced) or revealed to this browser.
   */
  readonly privacy?: "masked" | "revealed"
  /** lanstash masked the entry (a hidden game, shown to someone who is not an admin). */
  readonly hidden?: boolean
}

export interface Snapshot {
  readonly now_ms: number
  readonly started_ms: number
  /** Counter groups (requests, downloads, pass-through, eviction, https) by name. */
  readonly counters: Readonly<Record<string, Readonly<Record<string, number>>>>
  /** Rates of the byte counters, keyed "group.counter". */
  readonly rates_mb_s: Readonly<Record<string, number>>
  readonly downloads: readonly Download[]
  /** When the per-game usage was computed (it is cached for a while). */
  readonly usage_ms: number
  readonly usage: readonly UsageRow[]
  /** The last prefill, running or finished; absent from older lanstash. */
  readonly prefill?: Prefill | null
  /** Each platform's last prefill, by platform id; absent from older lanstash. */
  readonly prefills?: Readonly<Record<string, Prefill>>
  /** The last deletion of a game, running or done; absent from older lanstash. */
  readonly deleting?: Deleting | null
  /** When each platform's builds last changed (ms), by platform id; absent from older lanstash. */
  readonly builds_changed?: Readonly<Record<string, number>>
  /** The clients downloading now, fastest first; absent from older lanstash. */
  readonly clients?: readonly ClientNow[]
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v)

const isNumber = (v: unknown) => typeof v === "number" && Number.isFinite(v)

/**
 * Whether a value from lanstash is a snapshot as the page draws it (the fields it reads without
 * asking first): one that is not is skipped, so one odd event never blanks the page.
 */
export function isSnapshot(v: unknown): v is Snapshot {
  if (!isRecord(v)) return false
  if (!isNumber(v.now_ms) || !isNumber(v.started_ms) || !isNumber(v.usage_ms)) return false
  if (!isRecord(v.counters) || !Object.values(v.counters).every(isRecord)) return false
  if (!isRecord(v.rates_mb_s)) return false
  if (!Array.isArray(v.downloads) || !v.downloads.every((d) => isRecord(d) && typeof d.key === "string")) return false
  if (!Array.isArray(v.usage)) return false
  return v.usage.every((u) => isRecord(u) && typeof u.service === "string" && typeof u.game === "string")
}
