/**
 * Access: who uses the page and what they may do, from the frame down. With accounts on, what an
 * account may change follows its role's permissions, and a guest (not signed in) only reads, with
 * hidden games masked by lanstash; without them everyone may do everything, as before.
 *   - useAccess: the session, what it may do, and the actions.
 *   - Who: at the foot of the sidebar: Sign in, or the account (a link to its own page) with Sign out.
 *   - SignIn: the sign-in page (name and password; a wrong one makes the next try wait).
 *   - SignedIn: what a sign-in page says once signed in; the invite's page (./join) says it too.
 * The pages of the accounts are pieces of their own: ./own (your account), ./users (every account
 * and the invites, ./invites), ./join (making an account with an invite).
 */
import { createContext, type ReactNode, useContext, useState } from "react"
import { can, canChange, noSession, type Session, type SessionActions, withoutAccounts } from "@/contracts/session"
import { useT } from "@/i18n"
import { Button } from "@/styled/button"
import { Gate } from "@/styled/gate"
import { SideAccount } from "@/styled/sidebar"
import { Link } from "@/styled/text"
import { View } from "@/styled/view"

/** Where the sign-in page is. */
export const signInHome = "/sign-in"
/** Where one's own account's page is. */
export const accountHome = "/account"
/** Where the page of every account is. */
export const accountsHome = "/accounts"

export interface AccessValue {
  readonly session: Session
  /** The session may change things (an admin, or lanstash without accounts). */
  readonly canChange: boolean
  /** Whether the session may do `permission` (`prefill`, `prefill.own`, ...). */
  readonly can: (permission: string) => boolean
  readonly actions: SessionActions
  /** The address an invite's token is sent as. */
  readonly joinLink: (token: string) => string
}

/** An invite's address as a path of this page, when no origin is known. */
const joinPath = (token: string) => `/join/${token}`

// Without a provider, as lanstash without accounts: everyone may do everything.
const Context = createContext<AccessValue>({
  session: withoutAccounts,
  canChange: true,
  can: () => true,
  actions: noSession,
  joinLink: joinPath,
})

export function AccessProvider(props: {
  readonly session: Session
  readonly actions: SessionActions
  readonly joinLink?: (token: string) => string
  readonly children?: ReactNode
}) {
  const value = {
    session: props.session,
    canChange: canChange(props.session),
    can: (permission: string) => can(props.session, permission),
    actions: props.actions,
    joinLink: props.joinLink ?? joinPath,
  }
  return <Context.Provider value={value}>{props.children}</Context.Provider>
}

export function useAccess(): AccessValue {
  return useContext(Context)
}

/** Who uses the page, for the foot of the sidebar: nothing without accounts. */
export function Who() {
  const { session, actions } = useAccess()
  const t = useT("access")
  if (!session.accounts) return null
  if (session.role === "guest") {
    return (
      <SideAccount
        nobody={t("who.notSignedIn")}
        note={t("who.readingOnly")}
        action={<Link href={signInHome}>{t("who.signIn")}</Link>}
      />
    )
  }
  if (session.name === null) return <SideAccount name={t("who.thisMachine")} note={t("who.mayEverything")} />
  return (
    <SideAccount
      name={session.name}
      note={t(`roleName.${session.role}`)}
      href={accountHome}
      action={<Button onClick={() => void actions.signOut()}>{t("who.signOut")}</Button>}
    />
  )
}

/** What a sign-in page says once signed in: as whom, and the way on to `done`. */
export function SignedIn({ done }: { readonly done: string }) {
  const { session } = useAccess()
  const t = useT("access")
  return (
    <View
      title={t("signedIn.title")}
      subtitle={
        session.name === null
          ? t("signedIn.thisMachine")
          : t("signedIn.as", { name: session.name, role: t(`roleName.${session.role}`) })
      }
    >
      <Link href={done}>{t("signedIn.goOn")}</Link>
    </View>
  )
}

/** The sign-in page; `done` is where it goes once signed in. */
export function SignIn({ done }: { readonly done: string }) {
  const { session, actions } = useAccess()
  const t = useT("access")
  const [name, setName] = useState("")
  const [password, setPassword] = useState("")
  const [refused, setRefused] = useState<string>()
  const [busy, setBusy] = useState(false)
  if (session.role !== "guest") return <SignedIn done={done} />
  const go = async () => {
    setBusy(true)
    const r = await actions.signIn(name.trim(), password)
    setBusy(false)
    setPassword("")
    // Signed in, the session changes and this page says so, with the way on.
    setRefused("error" in r ? r.error : undefined)
  }
  return (
    <Gate
      title={t("signIn.title")}
      subtitle={t("signIn.subtitle")}
      fields={[
        { label: t("signIn.name"), value: name, onChange: setName, kind: "username" },
        { label: t("signIn.password"), value: password, onChange: setPassword, kind: "current-password" },
      ]}
      submit={busy ? t("signIn.busy") : t("signIn.submit")}
      onSubmit={() => void go()}
      busy={busy}
      error={refused}
      hint={t("signIn.hint")}
    />
  )
}
