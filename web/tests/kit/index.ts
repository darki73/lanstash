/**
 * Kit: the shared fakes of the tests: a snapshot to start from, live data for a resource, what
 * was kept over a range, and an EventSource the test drives.
 */

import type { Source } from "@/adapters/api/live"
import type { Series } from "@/contracts/stats"
import type { Snapshot } from "@/contracts/status"
import { type Kept, kept } from "@/features/status/kept"
import { type Range, ranges } from "@/features/status/range"
import type { Live } from "@/features/status/resource"

/** A quiet snapshot: one minute up, nothing in flight, nothing stored; override what a test needs. */
export function snapshot(over: Partial<Snapshot> = {}): Snapshot {
  return {
    now_ms: 1_000_060_000,
    started_ms: 1_000_000_000,
    counters: { requests: { hits: 3, misses: 1, hit_bytes: 4_000_000_000 } },
    rates_mb_s: {},
    downloads: [],
    usage_ms: 1_000_050_000,
    usage: [],
    ...over,
  }
}

/** Live data whose latest snapshot is the last one given (a quiet one when none is). */
export function live(...recent: Snapshot[]): Live {
  const all = recent.length === 0 ? [snapshot()] : recent
  return { snapshot: all[all.length - 1] ?? snapshot(), recent: all, keep: 120 }
}

/** The end of the ranges the tests keep: a whole minute. */
export const KEPT_TO = 1_000 * 3_600_000

/**
 * What was kept over a range ending at KEPT_TO: each counter's rise per bucket, the last bucket
 * the newest (a bucket left out is 0).
 */
export function keptOf(values: Readonly<Record<string, readonly number[]>>, range: Range = "day"): Kept {
  const step = ranges[range].step
  const names = Object.keys(values)
  const longest = Math.max(0, ...Object.values(values).map((v) => v.length))
  const buckets = Array.from({ length: longest }, (_, i) => ({
    at_ms: KEPT_TO - (longest - i) * step,
    values: names.map((n) => values[n]?.[i - (longest - (values[n]?.length ?? 0))] ?? 0),
  }))
  const series: Series = { step_ms: step, names, buckets }
  return kept(series, range, KEPT_TO)
}

export class FakeSource implements Source {
  readyState = 1
  closed = false
  readonly url: string
  private listeners = new Map<string, ((e: MessageEvent<string>) => void)[]>()
  constructor(url: string) {
    this.url = url
  }
  addEventListener(type: string, listener: (e: MessageEvent<string>) => void) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener])
  }
  close() {
    this.closed = true
    this.readyState = 2
  }
  emit(type: string, data = "{}") {
    for (const l of this.listeners.get(type) ?? []) l(new MessageEvent(type, { data }))
  }
}

/** sources records every source opened; later collects what was put off, to run by hand. */
export function harness() {
  const sources: FakeSource[] = []
  const open = (url: string) => {
    const s = new FakeSource(url)
    sources.push(s)
    return s
  }
  const pending: (() => void)[] = []
  const later = (run: () => void) => pending.push(run)
  return { sources, open, pending, later }
}
