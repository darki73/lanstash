/**
 * Session: asks lanstash's API who uses the page (GET /api/v1/session), signs in and out (POST,
 * DELETE /api/v1/session), manages its accounts (/api/v1/users) and invites (GET, POST
 * /api/v1/invites, DELETE /api/v1/invites/<id>), and joins with an invite (POST
 * /api/v1/invites/<token>, which signs the new account in). Names and passwords go in
 * headers, percent-encoded, never in the address. The browser keeps the session in a cookie of
 * lanstash's; a read that fails, or an answer that is not a session, is undefined, never an
 * exception. The requests are passed in.
 */
import type { SendWith } from "@/adapters/api/dns"
import type { Delete } from "@/adapters/api/stash"
import type { Fetch } from "@/adapters/api/version"

/** A session as the API answers it. */
export interface Session {
  readonly name: string | null
  readonly role: "admin" | "player" | "viewer" | "guest"
  readonly accounts: boolean
  readonly permissions?: readonly string[]
  readonly links?: readonly string[]
}

/** An account as the API lists it. */
export interface User {
  readonly name: string
  readonly role: "admin" | "player" | "viewer"
  readonly links?: readonly string[]
}

const roles: readonly string[] = ["admin", "player", "viewer"]

/** Why a sign-in failed, and how long to wait first (ms). */
export interface Refusal {
  readonly error: string
  readonly retryMs?: number
}

export interface SessionActions {
  readonly signIn: (name: string, password: string) => Promise<Session | Refusal>
  readonly signOut: () => Promise<void>
  readonly users: () => Promise<readonly User[] | string>
  readonly addUser: (name: string, role: User["role"], password: string) => Promise<string | undefined>
  readonly setRole: (name: string, role: User["role"]) => Promise<string | undefined>
  readonly setPassword: (name: string, password: string, old?: string) => Promise<string | undefined>
  readonly removeUser: (name: string) => Promise<string | undefined>
  readonly setLink: (name: string, link: string, on: boolean) => Promise<string | undefined>
  readonly invites: () => Promise<readonly Invite[] | string>
  readonly makeInvite: (role: User["role"], links: readonly string[], name?: string) => Promise<MadeInvite | string>
  readonly revokeInvite: (id: string) => Promise<string | undefined>
  readonly invite: (token: string) => Promise<InviteView | string>
  readonly join: (token: string, name: string, password: string) => Promise<Session | Refusal>
}

/** What an invite makes, as its link's page reads it. */
export interface InviteView {
  readonly role: User["role"]
  readonly links: readonly string[]
  readonly untilMs: number
  readonly name: string | null
}

/** An invite not used yet, as the API lists it. */
export interface Invite extends InviteView {
  readonly id: string
}

/** A new invite's token and when it runs out. */
export interface MadeInvite {
  readonly token: string
  readonly untilMs: number
}

const asView = (v: unknown): InviteView | undefined => {
  const i = v as { role?: unknown; links?: unknown; until_ms?: unknown; name?: unknown } | null
  if (!roles.includes(String(i?.role)) || typeof i?.until_ms !== "number") return undefined
  const links = Array.isArray(i.links) ? i.links.filter((l): l is string => typeof l === "string") : []
  const name = typeof i.name === "string" ? i.name : null
  return { role: i.role as User["role"], links, untilMs: i.until_ms, name }
}

const asInvite = (v: unknown): Invite | undefined => {
  const id = (v as { id?: unknown } | null)?.id
  const view = asView(v)
  return typeof id === "string" && view !== undefined ? { id, ...view } : undefined
}

const isSession = (v: unknown): v is Session => {
  const s = v as Record<string, unknown> | null
  return (
    typeof s === "object" &&
    s !== null &&
    (s.name === null || typeof s.name === "string") &&
    (roles.includes(String(s.role)) || s.role === "guest") &&
    typeof s.accounts === "boolean"
  )
}

const reason = (body: unknown, fallback: string) => {
  const error = (body as { error?: unknown } | null)?.error
  return typeof error === "string" ? error : fallback
}

const enc = encodeURIComponent

const noAccounts: Session = { name: null, role: "admin", accounts: false }

export async function readSession(fetch: Fetch, apiBase: string): Promise<Session | undefined> {
  try {
    const answer = await fetch(`${apiBase}/api/v1/session`)
    const body: unknown = await answer.json().catch(() => null)
    // A lanstash from before accounts has no such path: everyone may do everything there.
    if (!answer.ok && (body as { kind?: unknown } | null)?.kind === "not-found") return noAccounts
    return answer.ok && isSession(body) ? body : undefined
  } catch {
    return undefined
  }
}

const isUser = (v: unknown): v is User => {
  const u = v as { name?: unknown; role?: unknown } | null
  return typeof u?.name === "string" && roles.includes(String(u?.role))
}

export function sessionActions(
  fetch: Fetch,
  post: SendWith,
  put: SendWith,
  del: Delete,
  apiBase: string,
): SessionActions {
  const user = (name: string) => `${apiBase}/api/v1/users/${enc(name)}`
  const done = async (send: Promise<{ ok: boolean; json(): Promise<unknown> }>) => {
    try {
      const answer = await send
      return answer.ok ? undefined : reason(await answer.json().catch(() => null), "lanstash refused")
    } catch {
      return "lanstash did not answer"
    }
  }
  // Sends a name and password to `url`; signed in, the session as lanstash now says.
  const signInAt = async (url: string, name: string, password: string): Promise<Session | Refusal> => {
    try {
      const answer = await post(url, { "X-Lanstash-User": enc(name), "X-Lanstash-Password": enc(password) })
      const body = (await answer.json().catch(() => null)) as { retry_ms?: unknown } | null
      if (answer.ok) return (await readSession(fetch, apiBase)) ?? { error: "lanstash said nothing of the session" }
      const retry = typeof body?.retry_ms === "number" ? { retryMs: body.retry_ms } : {}
      return { error: reason(body, "lanstash refused"), ...retry }
    } catch {
      return { error: "lanstash did not answer" }
    }
  }
  const invites = `${apiBase}/api/v1/invites`
  return {
    signIn: (name, password) => signInAt(`${apiBase}/api/v1/session`, name, password),
    join: (token, name, password) => signInAt(`${invites}/${enc(token)}`, name, password),
    invites: async () => {
      try {
        const answer = await fetch(invites)
        const body = (await answer.json().catch(() => null)) as { invites?: unknown; kind?: unknown } | null
        // A lanstash from before invites has no such path: it has none.
        if (!answer.ok && body?.kind === "not-found") return []
        if (!answer.ok) return reason(body, "lanstash refused")
        if (!Array.isArray(body?.invites)) return "lanstash answered something else"
        return body.invites.map(asInvite).filter((i): i is Invite => i !== undefined)
      } catch {
        return "lanstash did not answer"
      }
    },
    invite: async (token) => {
      try {
        const answer = await fetch(`${invites}/${enc(token)}`)
        const body: unknown = await answer.json().catch(() => null)
        if (!answer.ok) return reason(body, "lanstash refused")
        return asView(body) ?? "lanstash answered something else"
      } catch {
        return "lanstash did not answer"
      }
    },
    makeInvite: async (role, links, name) => {
      try {
        const named = name === undefined || name === "" ? [] : [`name=${enc(name)}`]
        const query = [`role=${role}`, ...links.map((l) => `link=${enc(l)}`), ...named].join("&")
        const answer = await post(`${invites}?${query}`)
        const body = (await answer.json().catch(() => null)) as { token?: unknown; until_ms?: unknown } | null
        if (!answer.ok) return reason(body, "lanstash refused")
        return typeof body?.token === "string" && typeof body.until_ms === "number"
          ? { token: body.token, untilMs: body.until_ms }
          : "lanstash answered something else"
      } catch {
        return "lanstash did not answer"
      }
    },
    revokeInvite: (id) => done(del(`${invites}/${enc(id)}`)),
    signOut: async () => {
      await done(del(`${apiBase}/api/v1/session`))
    },
    users: async () => {
      try {
        const answer = await fetch(`${apiBase}/api/v1/users`)
        const body = (await answer.json().catch(() => null)) as { users?: unknown } | null
        if (!answer.ok) return reason(body, "lanstash refused")
        return Array.isArray(body?.users) && body.users.every(isUser) ? body.users : "lanstash answered something else"
      } catch {
        return "lanstash did not answer"
      }
    },
    addUser: (name, role, password) =>
      done(post(`${user(name)}?role=${role}`, { "X-Lanstash-Password": enc(password) })),
    setRole: (name, role) => done(put(`${user(name)}?role=${role}`)),
    setPassword: (name, password, old) =>
      done(
        put(user(name), {
          "X-Lanstash-Password": enc(password),
          ...(old === undefined ? {} : { "X-Lanstash-Old-Password": enc(old) }),
        }),
      ),
    removeUser: (name) => done(del(user(name))),
    setLink: (name, link, on) => done(put(`${user(name)}?${on ? "link" : "unlink"}=${enc(link)}`)),
  }
}
