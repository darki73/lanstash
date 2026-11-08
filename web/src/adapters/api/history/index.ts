/**
 * History: asks lanstash's API what was updated when (GET /api/v1/history), newest first. An answer
 * that is not a history (an error, an older lanstash without the route) is undefined, never an
 * exception; an event that is not one is left out. The fetch is passed in.
 */
import type { Fetch } from "@/adapters/api/version"

export interface HistoryAnswer {
  readonly events: readonly {
    readonly at_ms: number
    readonly platform: string
    readonly game: string
    readonly change: "added" | "new-build" | "current"
    readonly version: string
    readonly from: string | null
  }[]
}

type Event = HistoryAnswer["events"][number]

const changes: readonly string[] = ["added", "new-build", "current"]

const isEvent = (e: unknown): e is Event => {
  if (typeof e !== "object" || e === null) return false
  const v = e as Record<string, unknown>
  return (
    typeof v.at_ms === "number" &&
    typeof v.platform === "string" &&
    typeof v.game === "string" &&
    typeof v.change === "string" &&
    changes.includes(v.change) &&
    typeof v.version === "string" &&
    (v.from === null || typeof v.from === "string")
  )
}

export async function readHistory(fetch: Fetch, apiBase: string): Promise<HistoryAnswer | undefined> {
  try {
    const answer = await fetch(`${apiBase}/api/v1/history`)
    if (!answer.ok) return undefined
    const body: unknown = await answer.json()
    const events = (body as { events?: unknown } | null)?.events
    return Array.isArray(events) ? { events: events.filter(isEvent) } : undefined
  } catch {
    return undefined
  }
}
