/**
 * Stats: counters over time, kept by lanstash beyond a restart (GET /api/v1/stats). A series is how
 * much each named counter ("requests.hit_bytes") went up in each bucket of step_ms, oldest first;
 * lanstash leaves out buckets with nothing in them, and filled() puts them back as zeros. A
 * StatsReader asks for a series; noStats is the reader of a page that has none.
 */

export interface Series {
  readonly step_ms: number
  readonly names: readonly string[]
  readonly buckets: readonly { readonly at_ms: number; readonly values: readonly number[] }[]
}

export interface SeriesQuery {
  readonly names: readonly string[]
  readonly from_ms: number
  readonly to_ms: number
  readonly step_ms: number
}

/** Asks for a series; undefined when lanstash keeps none or could not answer. */
export type StatsReader = (query: SeriesQuery) => Promise<Series | undefined>

export const noStats: StatsReader = async () => undefined

/** The counters named `prefix…` that went up most in a range, most first. */
export type TopReader = (
  prefix: string,
  from_ms: number,
  to_ms: number,
  limit: number,
) => Promise<readonly { readonly name: string; readonly value: number }[] | undefined>

/** What each game's bytes sent to clients are counted as: `served/<platform>/<game>`. */
export const SERVED = "served/"

/** The platform and game a `served/<platform>/<game>` counter is about. */
export function servedGame(name: string): { readonly platform: string; readonly game: string } | undefined {
  if (!name.startsWith(SERVED)) return undefined
  const [platform, ...rest] = name.slice(SERVED.length).split("/")
  const game = rest.join("/")
  return platform && game ? { platform, game } : undefined
}

/**
 * The series' values for `name`, one per bucket from `from_ms` to `to_ms` (a bucket lanstash left
 * out is 0), oldest first.
 */
export function filled(series: Series, name: string, from_ms: number, to_ms: number): number[] {
  const i = series.names.indexOf(name)
  const step = series.step_ms
  if (i < 0 || step <= 0 || to_ms <= from_ms) return []
  const count = Math.ceil((to_ms - from_ms) / step)
  const out = new Array<number>(count).fill(0)
  // Buckets start at whole minutes from from_ms (rounded down to its minute).
  const start = Math.floor(from_ms / 60_000) * 60_000
  for (const b of series.buckets) {
    const at = Math.floor((b.at_ms - start) / step)
    if (at >= 0 && at < count) out[at] = b.values[i] ?? 0
  }
  return out
}

/** The sum of the series' values for `name`. */
export const total = (series: Series, name: string): number => {
  const i = series.names.indexOf(name)
  return i < 0 ? 0 : series.buckets.reduce((sum, b) => sum + (b.values[i] ?? 0), 0)
}
