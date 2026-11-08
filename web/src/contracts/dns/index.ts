/**
 * DNS: the network's DNS server as lanstash works with it (GET /api/v1/dns). The setup says which
 * kinds of server lanstash knows, which one is set up (where, over HTTP or HTTPS, as whom,
 * lanstash's own address; the password is never given back, only whether one is kept). Over HTTPS
 * the server's certificate is trusted by the public roots, by one pinned certificate, or not
 * checked; a refusal names an untrusted certificate's SHA-256 (read by untrustedIn) so it can be
 * pinned. Once one is set up, the report says how its rewrites stand against the names lanstash
 * wants (missing, there twice, and rewrites to lanstash's address for names it does not want, which
 * are left alone), which names of a cached
 * service clients looked up that no rewrite answered (they went to the internet), and which hosts
 * lanstash itself refused. The actions read it, set the server up (or only test a setup), forget it,
 * and apply the plan; each answers the page again, or why not. noDns is the actions of a page that
 * cannot reach lanstash's DNS API.
 */

export interface DnsKind {
  readonly id: string
  readonly name: string
  /** The port its admin API usually listens on, over HTTP and over HTTPS. */
  readonly port: number
  readonly https_port: number
  /** It signs in with a user name and a password (false: a password alone; absent from older lanstash). */
  readonly user?: boolean
}

export interface DnsConfig {
  readonly kind: string
  readonly host: string
  readonly port: number
  /** Its admin API is HTTPS. */
  readonly https: boolean
  /** The SHA-256 of the one certificate trusted (self-signed or privately issued). */
  readonly pin: string | null
  /** Its certificate is not checked at all. */
  readonly unchecked: boolean
  readonly user: string
  /** A name the game names point at, which points at lanstash's address; null: they answer the address. */
  readonly via?: string | null
  /** lanstash's address in the rewrites; null: the one its names' rewrites already answer. */
  readonly answer: string | null
}

export interface DnsSetup {
  readonly kinds: readonly DnsKind[]
  readonly config: DnsConfig | null
  readonly password_set: boolean
}

export interface DnsPlan {
  /** lanstash's address. */
  readonly answer: string
  /** What new rewrites answer: the address, or a name that leads to it. */
  readonly write_as: string
  /** The name the names point at, and whether it has no rewrite yet (Add creates it first). */
  readonly via?: string | null
  readonly via_missing?: boolean
  /** Every name wanted, with its service (steam, blizzard, …; absent from older lanstash). */
  readonly services?: Readonly<Record<string, string>>
  /** Where the address came from: the setup, or the server's rewrites. */
  readonly answer_from: "config" | "rewrites"
  readonly wanted: number
  readonly missing: readonly string[]
  /** Names with no rewrite of their own that the server sends to lanstash anyway, by the rule
   * that does ("another rewrite" for a wider rewrite); absent from older lanstash. */
  readonly covered?: Readonly<Record<string, string>>
  readonly duplicates: readonly string[]
  readonly extra: readonly string[]
}

export interface DnsMissed {
  readonly name: string
  readonly service: string
  readonly count: number
  readonly clients: readonly string[]
}

export interface DnsReport {
  readonly server: string
  readonly plan: DnsPlan | null
  readonly plan_error: string | null
  readonly missed: readonly DnsMissed[]
  readonly lookups: number
  readonly oldest_ms: number | null
  readonly lookups_error: string | null
  readonly refused: readonly { readonly host: string; readonly count: number }[]
  readonly refused_others: number
}

/** What the last action did: a test or a save of the setup, or an apply. */
export type DnsDone =
  | { readonly tested: boolean; readonly kept: boolean; readonly server: string; readonly rewrites: number }
  | { readonly added: number; readonly deduplicated: number }

export interface DnsPage {
  readonly setup: DnsSetup
  readonly report: DnsReport | null
  /** Why there is no report (such as no server set up yet). */
  readonly report_error: string | null
  readonly done: DnsDone | null
}

/** The page, or why there is none. */
export type DnsAnswer = DnsPage | string

export interface DnsActions {
  read(): Promise<DnsAnswer>
  /** Sets the server up (password: undefined keeps the kept one), or only tests the setup. */
  save(config: DnsConfig, password: string | undefined, test: boolean): Promise<DnsAnswer>
  forget(): Promise<DnsAnswer>
  apply(): Promise<DnsAnswer>
}

/** What precedes an untrusted certificate's SHA-256 in lanstash's words for a refusal. */
const untrustedMark = "untrusted certificate, SHA-256 "

/** The SHA-256 of the certificate a refusal says was not trusted, if it says one. */
export function untrustedIn(message: string): string | undefined {
  const at = message.indexOf(untrustedMark)
  if (at < 0) return undefined
  const fp = /^[0-9A-Fa-f:]+/.exec(message.slice(at + untrustedMark.length))?.[0]
  return fp === undefined || fp === "" ? undefined : fp
}

const unavailable = "the DNS server's setup is not available here"

export const noDns: DnsActions = {
  read: async () => unavailable,
  save: async () => unavailable,
  forget: async () => unavailable,
  apply: async () => unavailable,
}
