/**
 * Account panel: the platform accounts lanstash signs in to (Steam, Epic), so it can read what each
 * account's games need; several may be signed in, and what one owns lanstash fetches as that one.
 * Each account signed in is a row with its name, a mark when the platform refuses its kept sign-in
 * (it can revoke one: signing in with it again renews it), and Sign out. Sign in (Add an account,
 * once one is there) goes the platform's way:
 *   - by QR code (Steam): lanstash asks for a code and the panel shows it; scanned with the mobile
 *     app and approved there, it adds whoever approved it. While the code waits the panel says so
 *     (and once a phone has scanned it, that the approval is next); Cancel drops it.
 *   - by a code from the platform's own page (Epic): two steps, open the page (a new tab: sign in
 *     there), then paste the code it shows (or the whole page) and Sign in; Cancel closes them.
 * No password is typed into lanstash either way. Where lanstash offers no accounts for the
 * platform, the panel says so. Why a sign-in or sign-out failed is shown under the rest, all parts
 * spaced apart. Where the sign-in names a client (Epic), the last line says which (the launcher's
 * own, built in, or one set here; a panel of its own) and offers another, or the built-in one
 * back. With no account signed in, an empty box says so.
 */
import { useState } from "react"
import { type AccountActions, type AccountState, noAccount, signingIn } from "@/contracts/account"
import { useT } from "@/i18n"
import { Button, OutLink } from "@/styled/button"
import { Field, TextInput } from "@/styled/field"
import { Panel } from "@/styled/panel"
import { Scan } from "@/styled/scan"
import { Signal } from "@/styled/signal"
import { Stack } from "@/styled/stack"
import { Step, Steps } from "@/styled/steps"
import { Cell, Row, Table } from "@/styled/table"
import { Muted, Quiet } from "@/styled/text"

export interface AccountPanelProps {
  /** The platform's name ("Steam"). */
  readonly name: string
  /** The accounts, once lanstash said them; null where it offers none for the platform. */
  readonly state: AccountState | null | undefined
  readonly actions?: AccountActions
}

/**
 * Which client the sign-in names (Epic), a panel of its own laid out like a setting: the built-in
 * one, or one set here (a mark says which); Use another opens a box for `<id>:<secret>`, Use the
 * built-in one goes back.
 */
function ClientPanel({
  name,
  own,
  set,
}: {
  readonly name: string
  readonly own: boolean
  readonly set: (client: string | null) => Promise<string | undefined>
}) {
  const t = useT("platforms")
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState("")
  const [why, setWhy] = useState<string>()
  const save = (client: string | null) =>
    void set(client).then((w) => {
      setWhy(w)
      if (w === undefined) {
        setEditing(false)
        setDraft("")
      }
    })
  return (
    <Panel title={t("account.client.title")}>
      <Field
        label={t("account.client.label")}
        help={own ? t("account.client.ownHelp", { name }) : t("account.client.builtInHelp", { name })}
        badge={
          own ? (
            <Signal tone="waiting" still>
              {t("account.client.own")}
            </Signal>
          ) : (
            <Signal tone="good" still>
              {t("account.client.builtIn")}
            </Signal>
          )
        }
      >
        {editing ? (
          <>
            <TextInput
              label={t("account.client.input", { name })}
              value={draft}
              placeholder={t("account.client.placeholder")}
              secret
              onChange={setDraft}
              onEnter={() => save(draft)}
              onEscape={() => setEditing(false)}
            />
            <Button
              onClick={() => save(draft)}
              tone="accent"
              disabled={draft.trim() === ""}
              label={t("account.client.useLabel", { name })}
            >
              {t("account.client.use")}
            </Button>
          </>
        ) : own ? (
          <Button onClick={() => save(null)} label={t("account.client.builtInLabel", { name })}>
            {t("account.client.useBuiltIn")}
          </Button>
        ) : (
          <Button onClick={() => setEditing(true)} label={t("account.client.anotherLabel", { name })}>
            {t("account.client.another")}
          </Button>
        )}
      </Field>
      {why !== undefined && <Signal tone="bad">{why}</Signal>}
    </Panel>
  )
}

export function AccountPanel({ name, state, actions = noAccount }: AccountPanelProps) {
  const t = useT("platforms")
  const [refused, setRefused] = useState<string>()
  const [adding, setAdding] = useState(false)
  const [pasted, setPasted] = useState("")
  if (state === null) {
    return (
      <Panel title={t("account.title", { name })}>
        <Quiet title={t("account.offered.title", { name })} hint={t("account.offered.hint")} />
      </Panel>
    )
  }
  const view = state?.view
  const sign = view?.sign_in
  const accounts = view?.accounts ?? []
  const byCode = view?.method === "code"
  const act = (go: () => Promise<string | undefined>) => {
    setRefused(undefined)
    void go().then(setRefused)
  }
  const submit = () => {
    setRefused(undefined)
    void actions.signInWithCode(pasted).then((why) => {
      setRefused(why)
      if (why === undefined) {
        setAdding(false)
        setPasted("")
      }
    })
  }
  const start = () => (byCode ? setAdding(true) : act(actions.signIn))
  const action =
    view === undefined ? undefined : signingIn(view) || (byCode && adding) ? (
      <Button
        onClick={() => {
          setAdding(false)
          void actions.cancel()
        }}
      >
        {t("account.cancel")}
      </Button>
    ) : accounts.length > 0 ? (
      <Button onClick={start} label={t("account.addLabel", { name })}>
        {t("account.add")}
      </Button>
    ) : (
      <Button onClick={start} tone="accent" label={t("account.signInLabel", { name })}>
        {t("account.signIn")}
      </Button>
    )
  const accountsPanel = (
    <Panel title={t("account.title", { name })} action={action}>
      <Stack>
        {view === undefined && <Muted>{t("account.asking")}</Muted>}
        {sign?.state === "starting" && <Muted>{t("account.askingCode", { name })}</Muted>}
        {sign?.state === "waiting" && state !== undefined && (
          <Scan src={state.qr} alt={t("account.qr.alt", { name })}>
            {sign.scanned ? (
              <Signal tone="waiting">{t("account.qr.scanned")}</Signal>
            ) : (
              <Muted>{t("account.qr.scan")}</Muted>
            )}
            <Muted>{t("account.qr.noPassword")}</Muted>
          </Scan>
        )}
        {byCode && adding && view?.login_url !== undefined && (
          <Steps label={t("account.code.label", { name })}>
            <Step n={1} title={t("account.code.open.title", { name })}>
              <Muted>{t("account.code.open.text")}</Muted>
              <OutLink href={view.login_url}>{t("account.code.open.link", { name })}</OutLink>
            </Step>
            <Step n={2} title={t("account.code.paste.title")} last>
              <Muted>{t("account.code.paste.text")}</Muted>
              <TextInput
                label={t("account.code.paste.input", { name })}
                value={pasted}
                placeholder="authorizationCode"
                onChange={setPasted}
                onEnter={submit}
                onEscape={() => setPasted("")}
              />
              <Button
                onClick={submit}
                tone="accent"
                disabled={pasted.trim() === ""}
                label={t("account.code.paste.submitLabel", { name })}
              >
                {t("account.signIn")}
              </Button>
            </Step>
          </Steps>
        )}
        {accounts.length > 0 ? (
          <Table columns={[t("account.column.account"), t("account.column.signIn"), ""]}>
            {accounts.map((a) => (
              <Row key={a.id ?? a.name}>
                <Cell>{a.name}</Cell>
                <Cell>
                  {a.refused ? (
                    <Signal tone="bad" title={t("account.refusedHint", { name })}>
                      {t("account.refused")}
                    </Signal>
                  ) : (
                    <Signal tone="good" still>
                      {t("account.signedIn")}
                    </Signal>
                  )}
                </Cell>
                <Cell>
                  <Button
                    onClick={() => act(() => actions.signOut(a.id ?? a.name))}
                    label={t("account.signOutLabel", { account: a.name })}
                  >
                    {t("account.signOut")}
                  </Button>
                </Cell>
              </Row>
            ))}
          </Table>
        ) : (
          view !== undefined &&
          !signingIn(view) &&
          !adding && <Quiet title={t("account.none.title", { name })} hint={t("account.none.hint", { name })} />
        )}
        {sign?.state === "failed" && sign.error !== null && <Signal tone="bad">{sign.error}</Signal>}
        {refused !== undefined && <Signal tone="bad">{refused}</Signal>}
      </Stack>
    </Panel>
  )
  return (
    <>
      {accountsPanel}
      {view?.client !== undefined && <ClientPanel name={name} own={view.client === "own"} set={actions.setClient} />}
    </>
  )
}
