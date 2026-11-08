/**
 * Session: who uses the page (GET /api/v1/session): an admin, a viewer, or a guest (not signed in),
 * and whether lanstash has accounts at all (without them everyone is an admin, as before). An admin
 * changes things; a viewer and a guest read, with hidden games masked by lanstash. SessionActions
 * signs in and out, manages the accounts and the invites that let someone make their own (with a
 * role the admin chose), and joins with one; noSession is the page that cannot.
 */

export type Role = "admin" | "player" | "viewer" | "guest"

export interface Session {
  /** The account signed in; null for a guest, and on lanstash's own machine. */
  readonly name: string | null
  readonly role: Role
  /** lanstash has accounts (an admin among them); false: no sign-in, everyone may do everything. */
  readonly accounts: boolean
  /** What it may do (`prefill`, `prefill.own`, `settings`, ...); absent from an older lanstash. */
  readonly permissions?: readonly string[]
  /** The platform accounts linked to it (`steam/<account>`): their games are its own. */
  readonly links?: readonly string[]
}

/** Whether a session may do `permission` (anyone may, without accounts; an admin may everything). */
export const can = (s: Session, permission: string) =>
  !s.accounts || s.role === "admin" || (s.permissions ?? []).includes(permission)

/** Before lanstash said: nothing may be changed. */
export const unknownSession: Session = { name: null, role: "guest", accounts: true }

/** An account as an admin sees the list. */
export interface User {
  readonly name: string
  readonly role: "admin" | "player" | "viewer"
  /** Its linked platform accounts. */
  readonly links?: readonly string[]
}

/** Why a sign-in failed: the words, and how long to wait first (ms) when it was too many wrong ones. */
export interface Refusal {
  readonly error: string
  readonly retryMs?: number
}

/** What an invite makes, as the page its link opens sees it. */
export interface InviteView {
  readonly role: User["role"]
  readonly links: readonly string[]
  /** When it runs out (ms). */
  readonly untilMs: number
  /** The account's name, when the admin chose it; else the invited one chooses. */
  readonly name: string | null
}

/** An invite not used yet, as an admin sees the list (never its token). */
export interface Invite extends InviteView {
  readonly id: string
}

/** A new invite: the token its link holds (shown once), and when it runs out. */
export interface MadeInvite {
  readonly token: string
  readonly untilMs: number
}

export interface SessionActions {
  readonly signIn: (name: string, password: string) => Promise<Session | Refusal>
  readonly signOut: () => Promise<void>
  /** The accounts (an admin's), or why not. */
  readonly users: () => Promise<readonly User[] | string>
  /** Adds an account; why not, when it failed. */
  readonly addUser: (name: string, role: User["role"], password: string) => Promise<string | undefined>
  readonly setRole: (name: string, role: User["role"]) => Promise<string | undefined>
  /** A new password: an admin's for anyone, or one's own with the old password. */
  readonly setPassword: (name: string, password: string, old?: string) => Promise<string | undefined>
  readonly removeUser: (name: string) => Promise<string | undefined>
  /** Links a platform account (`steam/<account>`) to an account, or takes the link away. */
  readonly setLink: (name: string, link: string, on: boolean) => Promise<string | undefined>
  /** The invites not used yet (an admin's), or why not. */
  readonly invites: () => Promise<readonly Invite[] | string>
  /** Makes an invite for an account of `role` with `links` (and `name`, if chosen), or why not. */
  readonly makeInvite: (role: User["role"], links: readonly string[], name?: string) => Promise<MadeInvite | string>
  readonly revokeInvite: (id: string) => Promise<string | undefined>
  /** What the invite `token` makes, or why not (used, taken back, run out). */
  readonly invite: (token: string) => Promise<InviteView | string>
  /** Makes an account from an invite and signs it in, or why not. */
  readonly join: (token: string, name: string, password: string) => Promise<Session | Refusal>
}

const none = async () => "lanstash keeps no accounts"

export const noSession: SessionActions = {
  signIn: async () => ({ error: "lanstash keeps no accounts" }),
  signOut: async () => {},
  users: none,
  addUser: none,
  setRole: none,
  setPassword: none,
  removeUser: none,
  setLink: none,
  invites: none,
  makeInvite: none,
  revokeInvite: none,
  invite: none,
  join: async () => ({ error: "lanstash keeps no accounts" }),
}

/** Whether a session may change things. */
export const canChange = (s: Session) => !s.accounts || s.role === "admin"

/** lanstash without accounts: everyone may do everything. */
export const withoutAccounts: Session = { name: null, role: "admin", accounts: false }
