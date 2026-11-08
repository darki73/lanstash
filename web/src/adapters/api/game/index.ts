/**
 * Game: asks lanstash's API about one Battle.net game (GET /api/v1/games/blizzard/<game>). An
 * answer that is not a game (an error, a lanstash without the route) is undefined, never an
 * exception. The fetch is passed in.
 */
import type { Fetch } from "@/adapters/api/version"

interface Figures {
  readonly files_have: number
  readonly files_want: number
  readonly archived_have: number
  readonly archived_want: number
  readonly file_count: number
  readonly part_count: number
  readonly archive_count: number
  readonly requests: number
  readonly absent: number
}

interface Build {
  readonly version: string
  readonly build_config: string
  readonly have: number | null
  readonly want: number | null
  readonly complete: boolean
  readonly plan_error: string | null
  readonly detail: Figures | null
}

export interface GameAnswer {
  readonly game: string
  readonly product: string | null
  readonly region: string
  readonly build: Build | null
  readonly prerelease: Build | null
  readonly error: string | null
  readonly lancache?: boolean
  readonly flavours?: readonly {
    readonly product: string
    readonly name: string | null
    readonly build: Build | null
    readonly prerelease: Build | null
  }[]
  readonly installs?: {
    readonly platforms: readonly string[]
    readonly architectures: readonly string[]
    readonly locales?: readonly string[]
    readonly own?: string | null
    readonly held?: Readonly<Record<string, { readonly have: number; readonly want: number } | null>>
  } | null
}

const isAnswer = (v: unknown): v is GameAnswer =>
  typeof v === "object" &&
  v !== null &&
  typeof (v as { game?: unknown }).game === "string" &&
  typeof (v as { region?: unknown }).region === "string" &&
  "build" in v

export async function readGame(fetch: Fetch, apiBase: string, game: string): Promise<GameAnswer | undefined> {
  try {
    const answer = await fetch(`${apiBase}/api/v1/games/blizzard/${encodeURIComponent(game)}`)
    if (!answer.ok) return undefined
    const body = await answer.json()
    return isAnswer(body) ? body : undefined
  } catch {
    return undefined
  }
}
