/**
 * Hidden: asks lanstash's API which games are kept private (GET /api/v1/hidden) and hides or shows
 * one (PUT, DELETE /api/v1/hidden/<platform>/<game>). Reading gives null from a lanstash that keeps
 * none (404) and undefined for an answer that is not a list, never an exception; a change gives the
 * list after it, or why it failed. The fetches are passed in.
 */
import type { Fetch } from "@/adapters/api/version"

const list = (body: unknown): readonly string[] | undefined => {
  const hidden = (body as { hidden?: unknown } | null)?.hidden
  return Array.isArray(hidden) && hidden.every((g) => typeof g === "string") ? hidden : undefined
}

const reason = (body: unknown) => {
  const error = (body as { error?: unknown } | null)?.error
  return typeof error === "string" ? error : "lanstash refused"
}

export async function readHidden(fetch: Fetch, apiBase: string): Promise<readonly string[] | null | undefined> {
  try {
    const answer = await fetch(`${apiBase}/api/v1/hidden`)
    const body: unknown = await answer.json().catch(() => null)
    if (!answer.ok) return (body as { kind?: unknown } | null)?.kind === "not-found" ? null : undefined
    return list(body)
  } catch {
    return undefined
  }
}

export const hidingActions = (put: Fetch, del: Fetch, apiBase: string) => ({
  set: async (platform: string, game: string, hidden: boolean): Promise<readonly string[] | string> => {
    const at = `${apiBase}/api/v1/hidden/${encodeURIComponent(platform)}/${encodeURIComponent(game)}`
    try {
      const answer = await (hidden ? put(at) : del(at))
      const body: unknown = await answer.json().catch(() => null)
      if (!answer.ok) return reason(body)
      return list(body) ?? "lanstash answered something else"
    } catch {
      return "lanstash did not answer"
    }
  },
})
