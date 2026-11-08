/**
 * Settings: the settings the web UI may change, through lanstash's API: GET /api/v1/settings,
 * PUT /api/v1/settings/<key>?value=<value> (the value escaped), DELETE /api/v1/settings/<key>, and
 * POST /api/v1/restart[?force=1]. A refusal gives the API's reason (its error message); no answer
 * gives "lanstash did not answer". The requests are passed in: the browser's fetch in the app,
 * fakes in tests.
 */
/** One setting as the API answers it. */
export interface ApiSetting {
  readonly key: string
  readonly group: string
  readonly label: string
  readonly help: string
  readonly choices: readonly string[]
  readonly multi?: boolean
  readonly parts?: readonly { readonly label: string; readonly choices: readonly string[] }[]
  readonly custom?: { readonly label: string; readonly hint: string } | null
  readonly value: string | null
  readonly config: string | null
  readonly default: string | null
  readonly ui: string | null
  readonly flag: boolean
}

/** The settings, or why there are none. */
type Answer = readonly ApiSetting[] | string

/** One request of a kind (GET, PUT, DELETE, POST) to a URL. */
export type Send = (url: string) => Promise<{ readonly ok: boolean; json(): Promise<unknown> }>

const reason = (body: unknown) =>
  typeof body === "object" && body !== null && typeof (body as { error?: unknown }).error === "string"
    ? (body as { error: string }).error
    : "lanstash refused"

const isSetting = (v: unknown): v is ApiSetting =>
  typeof v === "object" &&
  v !== null &&
  ["key", "group", "label", "help"].every((k) => typeof (v as Record<string, unknown>)[k] === "string") &&
  Array.isArray((v as { choices?: unknown }).choices) &&
  typeof (v as { flag?: unknown }).flag === "boolean"

export function settingsActions(get: Send, put: Send, del: Send, post: Send, apiBase: string) {
  const at = (key = "") => `${apiBase}/api/v1/settings${key === "" ? "" : `/${encodeURIComponent(key)}`}`
  const ask = async (go: Send, url: string): Promise<Answer> => {
    try {
      const answer = await go(url)
      const body = await answer.json().catch(() => null)
      if (!answer.ok) return reason(body)
      const list = (body as { settings?: unknown } | null)?.settings
      return Array.isArray(list) && list.every(isSetting) ? list : "lanstash refused"
    } catch {
      return "lanstash did not answer"
    }
  }
  return {
    read: () => ask(get, at()),
    set: (key: string, value: string) => ask(put, `${at(key)}?value=${encodeURIComponent(value)}`),
    reset: (key: string) => ask(del, at(key)),
    async restart(force = false): Promise<string | undefined> {
      try {
        const answer = await post(`${apiBase}/api/v1/restart${force ? "?force=1" : ""}`)
        return answer.ok ? undefined : reason(await answer.json().catch(() => null))
      } catch {
        return "lanstash did not answer"
      }
    },
  }
}
