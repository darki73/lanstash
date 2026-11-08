/**
 * Account: the platform accounts lanstash signs in to (Steam, Epic), as GET
 * /api/v1/account/<platform> describes them: who is signed in (several may be: lanstash fetches
 * what each owns as that account; each says whether the platform refuses its kept sign-in), and
 * how one signs in: by a QR code that runs in lanstash while the page shows the code (Steam:
 * starting, waiting for the mobile app to scan and approve it, or failed and why; whoever approves
 * it is added), or by a code pasted from the platform's own page (Epic). The state carries where
 * the QR code's image is (another address each time it is read, so a rotated code shows). The
 * actions start and cancel a sign-in, sign in with a pasted code, and sign one account out;
 * noAccount is the actions of a page that cannot sign in.
 */

export type SignInState = "idle" | "starting" | "waiting" | "failed"

export interface SignedIn {
  readonly name: string
  /** What signing it out takes, where it is not the name (Epic's account id). */
  readonly id?: string
  /** The platform refuses its kept sign-in: it needs signing in again. */
  readonly refused: boolean
}

export interface AccountView {
  /** The accounts signed in, in the order they signed in. */
  readonly accounts: readonly SignedIn[]
  readonly sign_in: {
    readonly state: SignInState
    /** A phone has scanned the code; the sign-in waits for the approval. */
    readonly scanned: boolean
    readonly error: string | null
  }
  /** How one signs in: a QR code lanstash shows (Steam), or a code pasted from the platform's own page (Epic). */
  readonly method?: "qr" | "code"
  /** The platform's own page to sign in on, where one pastes a code from it. */
  readonly login_url?: string
  /** Which client the platform's sign-in names, where it names one (Epic): the built-in one, or one set here. */
  readonly client?: "built-in" | "own"
}

/** Each platform's accounts, by platform id: null where lanstash offers none. */
export type AccountsByPlatform = Readonly<Record<string, AccountState | null>>

export interface AccountState {
  readonly view: AccountView
  /** Where the waiting sign-in's QR code is. */
  readonly qr: string
}

export interface AccountActions {
  /** Starts a QR sign-in; undefined when it started, else why not. */
  signIn(): Promise<string | undefined>
  /** Signs in with what was pasted from the platform's own page; undefined when it did, else why not. */
  signInWithCode(pasted: string): Promise<string | undefined>
  cancel(): Promise<void>
  /** Forgets one account's kept sign-in (by its id, else its name); undefined when it went, else why not. */
  signOut(account: string): Promise<string | undefined>
  /** Sets the client the sign-in names (`<id>:<secret>`), or null for the built-in one; undefined when done, else why not. */
  setClient(client: string | null): Promise<string | undefined>
}

export const noAccount: AccountActions = {
  signIn: async () => "signing in is not available here",
  signInWithCode: async () => "signing in is not available here",
  setClient: async () => "the client cannot be changed here",
  cancel: async () => {},
  signOut: async () => "signing out is not available here",
}

/** Whether a sign-in is under way (asked for, or showing its code). */
export const signingIn = (v: AccountView | undefined) =>
  v?.sign_in.state === "starting" || v?.sign_in.state === "waiting"

/** Whether an account needs attention: none signed in, one refused, or a sign-in that failed. */
export const needsAttention = (v: AccountView | undefined) =>
  v !== undefined && (v.accounts.length === 0 || v.accounts.some((a) => a.refused) || v.sign_in.state === "failed")
