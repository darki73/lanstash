/**
 * DNS: the network's DNS server through lanstash's API: GET /api/v1/dns (the setup and the report),
 * PUT /api/v1/dns/setup?kind=&host=&port=&user=[&answer=][&https=1[&pin=|&unchecked=1]][&test=1] with the password in the
 * X-Lanstash-Password header (never in the address; left out, lanstash keeps the one it has),
 * DELETE /api/v1/dns/setup, and POST /api/v1/dns/apply. A refusal gives the API's reason (its error
 * message); no answer gives "lanstash did not answer". The requests are passed in: the browser's
 * fetch in the app, fakes in tests.
 */

/** One request of a kind (GET, PUT, DELETE, POST) to a URL, with extra headers. */
export type SendWith = (
  url: string,
  headers?: Readonly<Record<string, string>>,
) => Promise<{ readonly ok: boolean; json(): Promise<unknown> }>

/** The page as the API answers it. */
export interface ApiDnsPage {
  readonly setup: {
    readonly kinds: readonly {
      readonly id: string
      readonly name: string
      readonly port: number
      readonly https_port: number
    }[]
    readonly config: {
      readonly kind: string
      readonly host: string
      readonly port: number
      readonly https: boolean
      readonly pin: string | null
      readonly unchecked: boolean
      readonly user: string
      readonly answer: string | null
      readonly via?: string | null
    } | null
    readonly password_set: boolean
  }
  readonly report: {
    readonly server: string
    readonly plan: {
      readonly answer: string
      readonly write_as: string
      readonly via?: string | null
      readonly via_missing?: boolean
      readonly services?: Readonly<Record<string, string>>
      readonly answer_from: "config" | "rewrites"
      readonly wanted: number
      readonly missing: readonly string[]
      readonly duplicates: readonly string[]
      readonly extra: readonly string[]
    } | null
    readonly plan_error: string | null
    readonly missed: readonly {
      readonly name: string
      readonly service: string
      readonly count: number
      readonly clients: readonly string[]
    }[]
    readonly lookups: number
    readonly oldest_ms: number | null
    readonly lookups_error: string | null
    readonly refused: readonly { readonly host: string; readonly count: number }[]
    readonly refused_others: number
  } | null
  readonly report_error: string | null
  readonly done:
    | { readonly tested: boolean; readonly kept: boolean; readonly server: string; readonly rewrites: number }
    | { readonly added: number; readonly deduplicated: number }
    | null
}

/** A setup as the page sends it. */
export interface ApiDnsConfig {
  readonly kind: string
  readonly host: string
  readonly port: number
  readonly https: boolean
  readonly pin: string | null
  readonly unchecked: boolean
  readonly user: string
  readonly answer: string | null
  readonly via?: string | null
}

const reason = (body: unknown) =>
  typeof body === "object" && body !== null && typeof (body as { error?: unknown }).error === "string"
    ? (body as { error: string }).error
    : "lanstash refused"

const isPage = (v: unknown): v is ApiDnsPage => {
  const setup = (v as { setup?: unknown } | null)?.setup
  return (
    typeof setup === "object" &&
    setup !== null &&
    Array.isArray((setup as { kinds?: unknown }).kinds) &&
    typeof (setup as { password_set?: unknown }).password_set === "boolean"
  )
}

export function dnsActions(get: SendWith, put: SendWith, del: SendWith, post: SendWith, apiBase: string) {
  const at = (path: string) => `${apiBase}/api/v1/dns${path}`
  const ask = async (go: () => ReturnType<SendWith>): Promise<ApiDnsPage | string> => {
    try {
      const answer = await go()
      const body = await answer.json().catch(() => null)
      if (!answer.ok) return reason(body)
      return isPage(body) ? body : "lanstash refused"
    } catch {
      return "lanstash did not answer"
    }
  }
  return {
    read: () => ask(() => get(at(""))),
    save(config: ApiDnsConfig, password: string | undefined, test: boolean) {
      const q = new URLSearchParams({
        kind: config.kind,
        host: config.host,
        port: String(config.port),
        user: config.user,
        answer: config.answer ?? "",
      })
      if (config.via) q.set("via", config.via)
      if (config.https) {
        q.set("https", "1")
        if (config.unchecked) q.set("unchecked", "1")
        else if (config.pin !== null && config.pin !== "") q.set("pin", config.pin)
      }
      if (test) q.set("test", "1")
      const headers = password === undefined || password === "" ? undefined : { "X-Lanstash-Password": password }
      return ask(() => put(at(`/setup?${q.toString()}`), headers))
    },
    forget: () => ask(() => del(at("/setup"))),
    apply: () => ask(() => post(at("/apply"))),
  }
}
