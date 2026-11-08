/**
 * Updates: asks lanstash's API which games it keeps up to date on its own (GET
 * /api/v1/auto-update), as `<platform>/<game>`. Null when automatic updates are off (404); an
 * answer that is not a list (an error, an older lanstash) is undefined, never an exception. The
 * fetch is passed in.
 */
import type { Fetch } from "@/adapters/api/version"

export async function readFollowed(fetch: Fetch, apiBase: string): Promise<readonly string[] | null | undefined> {
  try {
    const answer = await fetch(`${apiBase}/api/v1/auto-update`)
    const body: unknown = await answer.json().catch(() => null)
    if (!answer.ok) return (body as { kind?: unknown } | null)?.kind === "not-found" ? null : undefined
    const followed = (body as { followed?: unknown } | null)?.followed
    return Array.isArray(followed) && followed.every((g) => typeof g === "string") ? followed : undefined
  } catch {
    return undefined
  }
}
