/**
 * Stash: deletes a game from lanstash's stash, or prunes its old builds (GET for what a prune would
 * delete, POST to start it: /api/v1/games/<platform>/<game>/prune), through its API (DELETE
 * /api/v1/games/<platform>/<game>). remove gives undefined when it went, else the API's reason (its
 * error message), or "lanstash did not answer". The DELETE is passed in: the browser's fetch in
 * the app, a fake in tests.
 */

/** The part of the browser's fetch (with method DELETE) this adapter uses. */
export type Delete = (url: string) => Promise<{ readonly ok: boolean; json(): Promise<unknown> }>

const reason = (body: unknown) =>
  typeof body === "object" && body !== null && typeof (body as { error?: unknown }).error === "string"
    ? (body as { error: string }).error
    : "lanstash refused"

const isPrunable = (v: unknown): v is { files: number; bytes: number; recent_bytes: number } =>
  typeof v === "object" &&
  v !== null &&
  ["files", "bytes", "recent_bytes"].every((k) => typeof (v as Record<string, unknown>)[k] === "number")

/** `del`, `post` and `get` send a DELETE, a POST and a GET; without `post` and `get`, no prune. */
export function stashActions(del: Delete, apiBase: string, post?: Delete, get?: Delete) {
  const at = (platform: string, game: string) =>
    `${apiBase}/api/v1/games/${encodeURIComponent(platform)}/${encodeURIComponent(game)}`
  const send = async (go: Delete, url: string): Promise<string | undefined> => {
    try {
      const answer = await go(url)
      return answer.ok ? undefined : reason(await answer.json().catch(() => null))
    } catch {
      return "lanstash did not answer"
    }
  }
  return {
    remove: (platform: string, game: string) => send(del, at(platform, game)),
    ...(post && get
      ? {
          prune: (platform: string, game: string) => send(post, `${at(platform, game)}/prune`),
          async prunable(platform: string, game: string) {
            try {
              const answer = await get(`${at(platform, game)}/prune`)
              const body = await answer.json().catch(() => null)
              if (!answer.ok) return reason(body)
              if (!isPrunable(body)) return "lanstash refused"
              const compact = (body as { compact?: unknown }).compact
              return {
                files: body.files,
                bytes: body.bytes,
                recent_bytes: body.recent_bytes,
                ...(typeof compact === "number" && compact > 0 ? { compact } : {}),
              }
            } catch {
              return "lanstash did not answer"
            }
          },
        }
      : {}),
  }
}
