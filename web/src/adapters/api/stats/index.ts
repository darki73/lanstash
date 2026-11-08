/**
 * Stats: asks lanstash's API for counters over time (GET /api/v1/stats?names=&from=&to=&step=). An
 * answer that is not a series (an error, an older lanstash without the route) is undefined, never
 * an exception. The fetch is passed in.
 */
import type { Fetch } from "@/adapters/api/version"

export interface StatsAnswer {
  readonly step_ms: number
  readonly names: readonly string[]
  readonly buckets: readonly { readonly at_ms: number; readonly values: readonly number[] }[]
}

const isAnswer = (v: unknown): v is StatsAnswer => {
  if (typeof v !== "object" || v === null) return false
  const a = v as Record<string, unknown>
  return (
    typeof a.step_ms === "number" &&
    Array.isArray(a.names) &&
    a.names.every((n) => typeof n === "string") &&
    Array.isArray(a.buckets) &&
    a.buckets.every(
      (b: unknown) =>
        typeof b === "object" &&
        b !== null &&
        typeof (b as { at_ms?: unknown }).at_ms === "number" &&
        Array.isArray((b as { values?: unknown }).values) &&
        (b as { values: unknown[] }).values.every((x) => typeof x === "number"),
    )
  )
}

/** The counters named `prefix…` that went up most (GET /api/v1/stats/top); undefined on any failure. */
export const topReader =
  (fetch: Fetch, apiBase: string) =>
  async (
    prefix: string,
    from_ms: number,
    to_ms: number,
    limit: number,
  ): Promise<{ name: string; value: number }[] | undefined> => {
    const q = `prefix=${encodeURIComponent(prefix)}&from=${Math.floor(from_ms)}&to=${Math.floor(to_ms)}&limit=${limit}`
    try {
      const answer = await fetch(`${apiBase}/api/v1/stats/top?${q}`)
      if (!answer.ok) return undefined
      const top = ((await answer.json()) as { top?: unknown } | null)?.top
      if (!Array.isArray(top)) return undefined
      return top.filter(
        (t): t is { name: string; value: number } =>
          typeof t === "object" && t !== null && typeof t.name === "string" && typeof t.value === "number",
      )
    } catch {
      return undefined
    }
  }

export const statsReader =
  (fetch: Fetch, apiBase: string) =>
  async (q: {
    readonly names: readonly string[]
    readonly from_ms: number
    readonly to_ms: number
    readonly step_ms: number
  }): Promise<StatsAnswer | undefined> => {
    const query = [
      `names=${q.names.map(encodeURIComponent).join(",")}`,
      `from=${Math.floor(q.from_ms)}`,
      `to=${Math.floor(q.to_ms)}`,
      `step=${Math.floor(q.step_ms)}`,
    ].join("&")
    try {
      const answer = await fetch(`${apiBase}/api/v1/stats?${query}`)
      if (!answer.ok) return undefined
      const body: unknown = await answer.json()
      return isAnswer(body) ? body : undefined
    } catch {
      return undefined
    }
  }
