/**
 * Health: asks lanstash's API for the checks it makes of itself (GET /api/v1/health). An answer
 * that is not a list of checks (an error, an older lanstash without the route) is undefined, never
 * an exception; a check that is not one is left out. The fetch is passed in.
 */
import type { Fetch } from "@/adapters/api/version"

export interface CheckAnswer {
  readonly level: "ok" | "warn" | "fail"
  readonly what: string
  readonly detail: string
}

const levels: readonly string[] = ["ok", "warn", "fail"]

const isCheck = (c: unknown): c is CheckAnswer => {
  if (typeof c !== "object" || c === null) return false
  const v = c as Record<string, unknown>
  return (
    typeof v.level === "string" &&
    levels.includes(v.level) &&
    typeof v.what === "string" &&
    typeof v.detail === "string"
  )
}

export const healthReader = (fetch: Fetch, apiBase: string) => async (): Promise<CheckAnswer[] | undefined> => {
  try {
    const answer = await fetch(`${apiBase}/api/v1/health`)
    if (!answer.ok) return undefined
    const checks = ((await answer.json()) as { checks?: unknown } | null)?.checks
    return Array.isArray(checks) ? checks.filter(isCheck) : undefined
  } catch {
    return undefined
  }
}
