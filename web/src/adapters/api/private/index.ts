/**
 * Private: asks lanstash's API which games the caller keeps private (GET /api/v1/private) and
 * marks or unmarks one (PUT, DELETE /api/v1/private/<platform>/<game>). Reading gives undefined
 * for anything that is not a list, never an exception; a change gives the list after it, or why it
 * failed. The fetches are passed in.
 */
import type { Fetch } from "@/adapters/api/version"

const list = (body: unknown): readonly string[] | undefined => {
  const games = (body as { private?: unknown } | null)?.private
  return Array.isArray(games) && games.every((g) => typeof g === "string") ? games : undefined
}

const reason = (body: unknown) => {
  const error = (body as { error?: unknown } | null)?.error
  return typeof error === "string" ? error : "lanstash refused"
}

export const privateActions = (fetch: Fetch, put: Fetch, del: Fetch, apiBase: string) => ({
  read: async (): Promise<readonly string[] | undefined> => {
    try {
      const answer = await fetch(`${apiBase}/api/v1/private`)
      if (!answer.ok) return undefined
      return list(await answer.json().catch(() => null))
    } catch {
      return undefined
    }
  },
  set: async (platform: string, game: string, on: boolean): Promise<readonly string[] | string> => {
    const at = `${apiBase}/api/v1/private/${encodeURIComponent(platform)}/${encodeURIComponent(game)}`
    try {
      const answer = await (on ? put(at) : del(at))
      const body: unknown = await answer.json().catch(() => null)
      if (!answer.ok) return reason(body)
      return list(body) ?? "lanstash answered something else"
    } catch {
      return "lanstash did not answer"
    }
  },
})
