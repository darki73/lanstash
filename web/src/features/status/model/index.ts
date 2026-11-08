/**
 * Status model: what the monitor shows, worked out from snapshots, as numbers (the page formats
 * them).
 *   - Series over the recent snapshots: a value per snapshot (a rate, a gauge), or a counter's
 *     change per second between neighbouring snapshots.
 *   - Downloads are grouped by object: lanstash fetches a large object as parallel pieces
 *     (16 MiB ranges side by side), each listed on its own in the snapshot; a group sums them.
 *   - Throughput, hit rates and storage of one snapshot.
 *   - Counter groups as rows: byte counters with their rates, gauges (a level, not a running
 *     total) without one.
 *   - Stored bytes per service, the largest first, with its games, the largest first.
 *   - A game's name as shown, the words for a hidden game where it is masked; the values of a
 *     plural form's words.
 */
import { HIDDEN_NAME } from "@/contracts/hidden"
import type { Download, Snapshot, UsageRow } from "@/contracts/status"
import { count } from "@/lib/count"

/** A game's name as shown: `hidden` (the words for a hidden game) where the name is masked. */
export const shownName = (name: string, hidden: string) => (name === HIDDEN_NAME ? hidden : name)

/** The values of a plural form's words: `count` picks the form, `n` is the number as shown. */
export const many = (n: number) => ({ count: n, n: count(n) })

/** counter is one counter of a snapshot, 0 when the group or the counter is not there. */
export function counter(s: Snapshot, group: string, name: string): number {
  return s.counters[group]?.[name] ?? 0
}

/** rate is a byte counter's rate in MB/s ("group.counter"), 0 before the first comparison. */
export function rate(s: Snapshot, key: string): number {
  return s.rates_mb_s[key] ?? 0
}

/** series is one value per snapshot, oldest first. */
export function series(recent: readonly Snapshot[], pick: (s: Snapshot) => number): number[] {
  return recent.map(pick)
}

/**
 * perSecond is a counter's change per second between each snapshot and the one before (the first
 * has none, and a counter that went down, after a restart, counts as 0).
 */
export function perSecond(recent: readonly Snapshot[], group: string, name: string): number[] {
  const out: number[] = []
  for (let i = 1; i < recent.length; i++) {
    const a = recent[i - 1]
    const b = recent[i]
    if (a === undefined || b === undefined) continue
    const secs = (b.now_ms - a.now_ms) / 1000
    const delta = counter(b, group, name) - counter(a, group, name)
    out.push(secs > 0 && delta > 0 ? delta / secs : 0)
  }
  return out
}

/** peak is the largest value, 0 for none. */
export function peak(values: readonly number[]): number {
  return values.reduce((a, b) => Math.max(a, b), 0)
}

export interface Throughput {
  /** MB/s to clients from the cache, to clients while downloading, and from the internet. */
  readonly fromCache: number
  readonly downloading: number
  readonly fromInternet: number
}

export function throughput(s: Snapshot): Throughput {
  return {
    fromCache: rate(s, "requests.hit_bytes"),
    downloading: rate(s, "requests.miss_bytes"),
    fromInternet: rate(s, "downloads.fetched_bytes"),
  }
}

export interface HitRates {
  /** Shares answered from the cache, 0 to 1: of requests, and of bytes; undefined before the first. */
  readonly objects: number | undefined
  readonly bytes: number | undefined
}

export function hitRates(s: Snapshot): HitRates {
  const share = (hit: number, miss: number) => (hit + miss === 0 ? undefined : hit / (hit + miss))
  return {
    objects: share(counter(s, "requests", "hits"), counter(s, "requests", "misses")),
    bytes: share(counter(s, "requests", "hit_bytes"), counter(s, "requests", "miss_bytes")),
  }
}

export interface Storage {
  /** Bytes in the store: the eviction gauge, or the per-game usage when eviction is off. */
  readonly stored: number
  /** Free bytes on the store's disk; undefined when eviction (which measures it) is off. */
  readonly free: number | undefined
  readonly evictedBytes: number
}

export function storage(s: Snapshot): Storage {
  const eviction = s.counters.eviction
  const used = s.usage.reduce((a, u) => a + u.bytes, 0)
  return {
    stored: eviction?.stored_bytes ?? used,
    free: eviction?.disk_free_bytes,
    evictedBytes: eviction?.evicted_bytes ?? 0,
  }
}

/** One piece of a download: its range and how much of it is in. */
export interface Piece {
  readonly start: number
  readonly length: number
  readonly done: number
}

/** A range that is known end to end: from, to, and how many of its bytes are in. */
export interface Ranged {
  readonly kind: "ranged"
  readonly from: number
  readonly to: number
  readonly done: number
  readonly length: number
}

/** A whole object or a piece without a known end: only the bytes received are known. */
export interface Open {
  readonly kind: "open"
  readonly received: number
}

export interface DownloadGroup {
  readonly key: string
  readonly total: number | null
  readonly progress: Ranged | Open
  /** The pieces, in order, when every one has a known range; empty otherwise. */
  readonly pieces: readonly Piece[]
  /** How many downloads the group holds. */
  readonly count: number
}

function piece(d: Download): Piece | undefined {
  if (d.span === null) return undefined
  const [start, end] = d.span
  if (end === null) return undefined
  const length = end - start
  return { start, length, done: Math.min(length, Math.max(0, d.received - start)) }
}

const sum = (values: readonly number[]) => values.reduce((a, b) => a + b, 0)

/** groupDownloads gives one group per object, in the order the objects first appear. */
export function groupDownloads(list: readonly Download[]): DownloadGroup[] {
  const byKey = new Map<string, Download[]>()
  for (const d of list) byKey.set(d.key, [...(byKey.get(d.key) ?? []), d])
  return [...byKey].map(([key, ds]) => {
    const sorted = [...ds].sort((a, b) => (a.span?.[0] ?? 0) - (b.span?.[0] ?? 0))
    const total = sorted[0]?.total ?? null
    const maybe = sorted.map(piece)
    const pieces = maybe.filter((p): p is Piece => p !== undefined)
    if (pieces.length < maybe.length) {
      return {
        key,
        total,
        count: ds.length,
        pieces: [],
        progress: { kind: "open", received: sum(ds.map((d) => d.received)) },
      }
    }
    const from = pieces[0]?.start ?? 0
    const to = Math.max(from, ...pieces.map((p) => p.start + p.length))
    const progress: Ranged = {
      kind: "ranged",
      from,
      to,
      done: sum(pieces.map((p) => p.done)),
      length: sum(pieces.map((p) => p.length)),
    }
    return { key, total, count: ds.length, pieces, progress }
  })
}

/** Objects being downloaded (a download in pieces counts once). */
export function objectsInFlight(s: Snapshot): number {
  return new Set(s.downloads.map((d) => d.key)).size
}

/** Counters that hold a level rather than a running total: no rate. */
const gauges = new Set([
  "eviction.stored_bytes",
  "eviction.disk_free_bytes",
  "eviction.short_bytes",
  "https.active",
  "storage.segments",
  "storage.object_files",
  "storage.objects",
])

/** Whether a counter ("group.counter") holds a level rather than a running total. */
export const isGauge = (key: string) => gauges.has(key)

export interface CounterRow {
  /** The counter as lanstash names it ("hit_bytes"). */
  readonly key: string
  /** The same in words ("hit bytes"). */
  readonly name: string
  readonly value: number
  /** A byte count, shown as a size. */
  readonly bytes: boolean
  /** MB/s, for a byte counter that is a running total. */
  readonly rate: number | undefined
}

export interface CounterGroup {
  readonly name: string
  readonly rows: readonly CounterRow[]
}

/** counterGroups gives the named counter groups of a snapshot that it has, in the order asked. */
export function counterGroups(s: Snapshot, names: readonly string[]): CounterGroup[] {
  return names.flatMap((name) => {
    const values = s.counters[name]
    if (values === undefined) return []
    const rows = Object.entries(values).map(([counterName, value]) => {
      const key = `${name}.${counterName}`
      const bytes = counterName.includes("bytes")
      return {
        key: counterName,
        name: counterName.replace(/_/g, " "),
        value,
        bytes,
        rate: bytes && !gauges.has(key) ? (s.rates_mb_s[key] ?? 0) : undefined,
      }
    })
    return [{ name, rows }]
  })
}

export interface StoredService {
  readonly service: string
  readonly objects: number
  readonly bytes: number
  readonly games: readonly UsageRow[]
}

/** storedServices sums the usage per service; services and games come largest first. */
export function storedServices(usage: readonly UsageRow[]): StoredService[] {
  const byService = new Map<string, UsageRow[]>()
  for (const u of usage) byService.set(u.service, [...(byService.get(u.service) ?? []), u])
  const largest = (a: { bytes: number }, b: { bytes: number }) => b.bytes - a.bytes
  return [...byService]
    .map(([service, games]) => ({
      service,
      objects: sum(games.map((g) => g.objects)),
      bytes: sum(games.map((g) => g.bytes)),
      games: [...games].sort(largest),
    }))
    .sort(largest)
}
