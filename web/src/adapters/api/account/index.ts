/**
 * Account: a platform's accounts through lanstash's API: GET /api/v1/account/<platform> for the
 * accounts and the sign-in (from a lanstash that kept one account, that one as the list; null where
 * lanstash offers no accounts for the platform), POST .../sign-in to start a QR sign-in, or with a
 * code pasted from the platform's own page to sign in with it, DELETE .../sign-in to cancel a
 * sign-in, POST .../sign-out?account=<id or name> to forget one account's sign-in, POST
 * .../client with <id>:<secret> and DELETE .../client to set the client the sign-in names or go
 * back to the built-in one, and .../qr.svg for the QR code (with a turn in the query so a rotated
 * code is fetched again). A code and a secret go percent-encoded in the X-Lanstash-Secret header,
 * never in the address. An answer that is not an account is undefined, never an exception. The
 * fetch, POST and DELETE are passed in: the browser's in the app, fakes in tests.
 */
/** The part of the browser's fetch (GET, POST or DELETE) this adapter uses. */
export type Send = (
  url: string,
  headers?: Readonly<Record<string, string>>,
) => Promise<{ readonly ok: boolean; json(): Promise<unknown> }>

/** The header a code or a client's secret goes in. */
export const SECRET_HEADER = "X-Lanstash-Secret"

const secret = (value: string) => ({ [SECRET_HEADER]: encodeURIComponent(value.trim()) })

/** What GET /api/v1/account/<platform> answers. */
export interface AccountAnswer {
  readonly accounts: readonly { readonly name: string; readonly id?: string; readonly refused: boolean }[]
  readonly sign_in: {
    readonly state: "idle" | "starting" | "waiting" | "failed"
    readonly scanned: boolean
    readonly error: string | null
  }
  readonly method?: "qr" | "code"
  readonly login_url?: string
  readonly client?: "built-in" | "own"
}

const states = new Set(["idle", "starting", "waiting", "failed"])

const isSignedIn = (v: unknown) =>
  typeof v === "object" &&
  v !== null &&
  typeof (v as { name?: unknown }).name === "string" &&
  typeof (v as { refused?: unknown }).refused === "boolean" &&
  ["string", "undefined"].includes(typeof (v as { id?: unknown }).id)

/** The answer as this page reads it: from a lanstash of one account (`account` alone), that one listed. */
const asView = (v: unknown): AccountAnswer | undefined => {
  if (typeof v !== "object" || v === null || !isView(v)) return undefined
  const a = v as { accounts?: unknown; account?: unknown; method?: unknown; login_url?: unknown }
  if (a.method !== undefined && a.method !== "qr" && a.method !== "code") return undefined
  if (a.login_url !== undefined && typeof a.login_url !== "string") return undefined
  const client = (v as { client?: unknown }).client
  if (client !== undefined && client !== "built-in" && client !== "own") return undefined
  if (Array.isArray(a.accounts)) return a.accounts.every(isSignedIn) ? (v as AccountAnswer) : undefined
  if (!(a.account === null || typeof a.account === "string")) return undefined
  const accounts = typeof a.account === "string" ? [{ name: a.account, refused: false }] : []
  return { ...(v as Omit<AccountAnswer, "accounts">), accounts }
}

const isView = (v: unknown): boolean => {
  if (typeof v !== "object" || v === null) return false
  const a = v as { sign_in?: { state?: unknown; scanned?: unknown; error?: unknown } }
  return (
    typeof a.sign_in === "object" &&
    a.sign_in !== null &&
    states.has(a.sign_in.state as string) &&
    typeof a.sign_in.scanned === "boolean" &&
    (a.sign_in.error === null || typeof a.sign_in.error === "string")
  )
}

const reason = (body: unknown) =>
  typeof body === "object" && body !== null && typeof (body as { error?: unknown }).error === "string"
    ? (body as { error: string }).error
    : "lanstash refused"

const notFound = (body: unknown) =>
  typeof body === "object" && body !== null && (body as { kind?: unknown }).kind === "not-found"

export function accountActions(get: Send, post: Send, del: Send, apiBase: string) {
  const at = (platform: string) => `${apiBase}/api/v1/account/${encodeURIComponent(platform)}`
  const send = async (go: Send, url: string, headers?: Record<string, string>): Promise<string | undefined> => {
    try {
      const answer = await (headers === undefined ? go(url) : go(url, headers))
      return answer.ok ? undefined : reason(await answer.json().catch(() => null))
    } catch {
      return "lanstash did not answer"
    }
  }
  return {
    /** The accounts; null where lanstash offers none for the platform; undefined when unknown. */
    async read(platform: string): Promise<AccountAnswer | null | undefined> {
      try {
        const answer = await get(at(platform))
        const body = await answer.json().catch(() => null)
        if (!answer.ok) return notFound(body) ? null : undefined
        return asView(body)
      } catch {
        return undefined
      }
    },
    signIn: (platform: string) => send(post, `${at(platform)}/sign-in`),
    signInWithCode: (platform: string, pasted: string) => send(post, `${at(platform)}/sign-in`, secret(pasted)),
    cancel: async (platform: string) => {
      await send(del, `${at(platform)}/sign-in`)
    },
    signOut: (platform: string, account: string) =>
      send(post, `${at(platform)}/sign-out?account=${encodeURIComponent(account)}`),
    setClient: (platform: string, client: string | null) =>
      client === null ? send(del, `${at(platform)}/client`) : send(post, `${at(platform)}/client`, secret(client)),
    qr: (platform: string, turn: number) => `${at(platform)}/qr.svg?turn=${turn}`,
  }
}
