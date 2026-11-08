/**
 * Builds: asks lanstash's API for a platform's builds (GET /api/v1/builds/<platform>: Battle.net's,
 * Steam's, in one shape). An answer
 * that is not a builds list (an error, an older lanstash without the route) is undefined, never
 * an exception. The fetch is passed in.
 */
import type { Fetch } from "@/adapters/api/version"

/** A build and how much of it is stashed. */
interface Release {
  readonly version: string
  readonly build_config: string
  readonly have: number | null
  readonly want: number | null
  readonly complete: boolean
  readonly plan_error: string | null
}

export interface BuildsAnswer {
  readonly region: string
  readonly lancache?: boolean
  readonly games: readonly {
    readonly game: string
    readonly product: string | null
    readonly version: string | null
    readonly build_config: string | null
    readonly have?: number | null
    readonly want?: number | null
    readonly plan_error?: string | null
    readonly current: boolean
    readonly prerelease?: Release | null
    readonly error: string | null
    readonly lancache_share?: { readonly have: number; readonly want: number } | null
    readonly flavours?: readonly {
      readonly product: string
      readonly name: string | null
      readonly build: Release | null
      readonly prerelease: Release | null
    }[]
  }[]
}

const isGame = (g: unknown) =>
  typeof g === "object" &&
  g !== null &&
  typeof (g as { game?: unknown }).game === "string" &&
  typeof (g as { current?: unknown }).current === "boolean"

const isAnswer = (v: unknown): v is BuildsAnswer =>
  typeof v === "object" &&
  v !== null &&
  typeof (v as { region?: unknown }).region === "string" &&
  Array.isArray((v as { games?: unknown }).games) &&
  (v as { games: unknown[] }).games.every(isGame)

export async function readBuilds(
  fetch: Fetch,
  apiBase: string,
  platform = "blizzard",
): Promise<BuildsAnswer | undefined> {
  try {
    const answer = await fetch(`${apiBase}/api/v1/builds/${encodeURIComponent(platform)}`)
    if (!answer.ok) return undefined
    const body = await answer.json()
    return isAnswer(body) ? body : undefined
  } catch {
    return undefined
  }
}
