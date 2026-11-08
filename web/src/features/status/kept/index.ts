/**
 * Kept: what lanstash kept of its counters over the range the overview shows (by the minute, beyond
 * a restart), read as a whole: each counter's value per bucket and its total, and when the last
 * bucket was that saw any of some counters go up. useKept reads it for a range (in as many asks as
 * lanstash takes the counters in, put back together) and reads it again as each new minute comes;
 * useTop reads the games sent most over the same range.
 */
import { useEffect, useState } from "react"
import { filled, type Series, type StatsReader, type TopReader, total } from "@/contracts/stats"
import { type Range, type RangeSpec, ranges } from "@/features/status/range"

export interface Kept {
  readonly range: Range
  readonly spec: RangeSpec
  /** The range's start and end (ms). */
  readonly from: number
  readonly to: number
  /** A counter's rise per bucket, oldest first (a bucket with nothing is 0). */
  readonly values: (name: string) => number[]
  /** A counter's rise over the whole range. */
  readonly total: (name: string) => number
  /** The end (ms) of the last bucket where any of `names` rose; 0 when none did. */
  readonly lastRise: (names: readonly string[]) => number
}

/** A series over `range` ending at `to`, read as a whole. */
export function kept(series: Series, range: Range, to: number): Kept {
  const spec = ranges[range]
  const from = to - spec.span
  return {
    range,
    spec,
    from,
    to,
    values: (name) => filled(series, name, from, to),
    total: (name) => total(series, name),
    lastRise: (names) => {
      const at = names.map((n) => series.names.indexOf(n)).filter((i) => i >= 0)
      const rose = series.buckets.filter((b) => at.some((i) => (b.values[i] ?? 0) > 0))
      const last = rose.reduce((m, b) => Math.max(m, b.at_ms), 0)
      return last === 0 ? 0 : Math.min(to, last + series.step_ms)
    },
  }
}

/** The most counters lanstash answers for in one series. */
export const NAMES_PER_ASK = 32

/** Series over the same range and buckets, one: their names side by side, bucket by bucket. */
export function merged(parts: readonly Series[]): Series | undefined {
  const first = parts[0]
  if (first === undefined) return undefined
  const names = parts.flatMap((p) => p.names)
  const at = [...new Set(parts.flatMap((p) => p.buckets.map((b) => b.at_ms)))].sort((a, b) => a - b)
  const buckets = at.map((at_ms) => ({
    at_ms,
    values: parts.flatMap((p) => p.buckets.find((b) => b.at_ms === at_ms)?.values ?? p.names.map(() => 0)),
  }))
  return { step_ms: first.step_ms, names, buckets }
}

/** Asks for `names` in as many series as lanstash takes them, and puts the answers together. */
async function askAll(stats: StatsReader, names: readonly string[], from: number, to: number, step: number) {
  const asks: string[][] = []
  for (let i = 0; i < names.length; i += NAMES_PER_ASK) asks.push(names.slice(i, i + NAMES_PER_ASK))
  const got = await Promise.all(asks.map((n) => stats({ names: n, from_ms: from, to_ms: to, step_ms: step })))
  return got.some((s) => s === undefined) ? undefined : merged(got.filter((s) => s !== undefined))
}

/** The end of a range: now, to the minute, so a re-render each second asks nothing new. */
export const minuteOf = (now: number) => Math.floor(now / 60_000) * 60_000

/**
 * The counters `names` over `range`, read again each minute: undefined while the first answer is
 * awaited (or the range changed), null when lanstash keeps none.
 */
export function useKept(
  stats: StatsReader | undefined,
  range: Range,
  now: number,
  names: readonly string[],
): Kept | null | undefined {
  const to = minuteOf(now)
  const key = names.join(",")
  const [got, setGot] = useState<{ range: Range; key: string; kept: Kept | null }>()
  useEffect(() => {
    if (stats === undefined) return
    let live = true
    const spec = ranges[range]
    void askAll(stats, key.split(","), to - spec.span, to, spec.step).then((s) => {
      if (live) setGot({ range, key, kept: s === undefined ? null : kept(s, range, to) })
    })
    return () => {
      live = false
    }
  }, [stats, range, to, key])
  if (stats === undefined) return null
  return got?.range === range && got.key === key ? got.kept : undefined
}

/** How many store directories are asked for, to add up into the games sent most. */
const TOP_DIRS = 24

/** The counters named `prefix…` that rose most over `range` (empty while read or when none are kept). */
export function useTop(
  top: TopReader | undefined,
  prefix: string,
  range: Range,
  now: number,
): readonly { readonly name: string; readonly value: number }[] {
  const to = minuteOf(now)
  const [got, setGot] = useState<{ range: Range; list: readonly { name: string; value: number }[] }>()
  useEffect(() => {
    if (top === undefined) return
    let live = true
    void top(prefix, to - ranges[range].span, to, TOP_DIRS).then((t) => {
      if (live) setGot({ range, list: t ?? [] })
    })
    return () => {
      live = false
    }
  }, [top, prefix, range, to])
  return got?.range === range ? got.list : []
}
