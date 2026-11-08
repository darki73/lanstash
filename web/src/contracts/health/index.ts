/**
 * Health: the checks the service makes of itself (GET /api/v1/health), each a level, what it is
 * about and the detail: what the store holds and what loading it could not take, the files the
 * service has open against its limit, the store disk's free space, what the background check
 * found. A HealthReader asks for them; noHealth is the reader of a page that has none.
 */

export type HealthLevel = "ok" | "warn" | "fail"

export interface HealthCheck {
  readonly level: HealthLevel
  readonly what: string
  readonly detail: string
}

/** The checks now; undefined when lanstash could not say (or keeps none). */
export type HealthReader = () => Promise<readonly HealthCheck[] | undefined>

export const noHealth: HealthReader = async () => undefined

/** The worst level among `checks` ("ok" when there are none). */
export function worst(checks: readonly HealthCheck[]): HealthLevel {
  if (checks.some((c) => c.level === "fail")) return "fail"
  if (checks.some((c) => c.level === "warn")) return "warn"
  return "ok"
}
