/**
 * Prefill: starts and stops prefill jobs through lanstash's API (POST
 * /api/v1/prefill/<platform>/<game>, with build=pre for a pre-release build, from=lancache to take
 * what lancache's old cache holds first, from=lancache-only to take only that, install=<tags> for
 * another platform's install; POST
 * /api/v1/prefill/stop[?platform=<id>]; DELETE /api/v1/prefill/<platform>/<game> takes a game out
 * of the queue). start and unqueue give undefined when the
 * job started, else the API's reason (its error message), or "lanstash did not answer". The POST
 * is passed in: the browser's fetch in the app, a fake in tests.
 */

/** The part of the browser's fetch (with method POST, or DELETE) this adapter uses; `headers` for what must not be in the address. */
export type Post = (
  url: string,
  headers?: Readonly<Record<string, string>>,
) => Promise<{ readonly ok: boolean; json(): Promise<unknown> }>

const reason = (body: unknown) =>
  typeof body === "object" && body !== null && typeof (body as { error?: unknown }).error === "string"
    ? (body as { error: string }).error
    : "lanstash refused"

/** `post` sends a POST; `del` a DELETE (without it, nothing can be taken out of a queue). */
export function prefillActions(post: Post, apiBase: string, del?: Post) {
  const at = (platform: string, game: string) =>
    `${apiBase}/api/v1/prefill/${encodeURIComponent(platform)}/${encodeURIComponent(game)}`
  return {
    async start(
      platform: string,
      game: string,
      prerelease = false,
      fromLancache: boolean | "only" = false,
      queue = false,
      install?: string,
    ): Promise<string | undefined> {
      const from = fromLancache === "only" ? "from=lancache-only" : fromLancache && "from=lancache"
      const other = install !== undefined && `install=${encodeURIComponent(install)}`
      const query = [prerelease && "build=pre", from, queue && "queue=1", other].filter(Boolean).join("&")
      try {
        const answer = await post(`${at(platform, game)}${query ? `?${query}` : ""}`)
        return answer.ok ? undefined : reason(await answer.json().catch(() => null))
      } catch {
        return "lanstash did not answer"
      }
    },
    async unqueue(platform: string, game: string): Promise<string | undefined> {
      if (del === undefined) return "lanstash cannot take games out of the queue here"
      try {
        const answer = await del(at(platform, game))
        return answer.ok ? undefined : reason(await answer.json().catch(() => null))
      } catch {
        return "lanstash did not answer"
      }
    },
    async stop(platform?: string): Promise<void> {
      try {
        const only = platform === undefined ? "" : `?platform=${encodeURIComponent(platform)}`
        await post(`${apiBase}/api/v1/prefill/stop${only}`)
      } catch {
        // The next snapshot shows whether it stopped.
      }
    },
  }
}
