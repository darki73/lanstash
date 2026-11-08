/**
 * Version: asks lanstash's API which version is running (GET /api/v1/version). An answer that is
 * not a version (an error, an older lanstash without the route) is undefined, never an exception:
 * the page then simply shows no version. The fetch is passed in: the browser's in the app, a fake
 * in tests.
 */

/** The part of the browser's fetch this adapter uses. */
export type Fetch = (url: string) => Promise<{ readonly ok: boolean; json(): Promise<unknown> }>

export interface VersionAnswer {
  readonly version: string
  readonly latest?: string | null
}

const isAnswer = (v: unknown): v is VersionAnswer =>
  typeof v === "object" && v !== null && typeof (v as { version?: unknown }).version === "string"

export async function readVersion(fetch: Fetch, apiBase: string): Promise<VersionAnswer | undefined> {
  try {
    const answer = await fetch(`${apiBase}/api/v1/version`)
    if (!answer.ok) return undefined
    const body = await answer.json()
    return isAnswer(body) ? body : undefined
  } catch {
    return undefined
  }
}
