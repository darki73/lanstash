/**
 * Own: the page of one's own account (/account), for anyone signed in: the role and what it lets
 * the account do, in words; the platform accounts linked to it (whose games it may prefill, with
 * prefill.own); and a new password, with the old one (every page signed in as it then signs out).
 * A guest is pointed to the sign-in; this machine is told it may do everything.
 */
import { useState } from "react"
import { accountsHome, signInHome, useAccess } from "@/features/access"
import { useT } from "@/i18n"
import { Button } from "@/styled/button"
import { Field, TextInput } from "@/styled/field"
import { Panel } from "@/styled/panel"
import { Signal } from "@/styled/signal"
import { Stack } from "@/styled/stack"
import { Tags } from "@/styled/tags"
import { Link, Muted, Quiet } from "@/styled/text"
import { View } from "@/styled/view"

/** The permissions a page of one's own may list, each with the key of its words. */
export const permissions = [
  ["prefill", "prefill"],
  ["prefill.own", "prefillOwn"],
  ["settings", "settings"],
  ["dns", "dns"],
  ["delete", "delete"],
  ["hide", "hide"],
  ["clients", "clients"],
  ["platforms", "platforms"],
  ["accounts", "accounts"],
] as const

function OwnPassword({ name }: { readonly name: string }) {
  const { actions } = useAccess()
  const t = useT("access")
  const [old, setOld] = useState("")
  const [fresh, setFresh] = useState("")
  const [said, setSaid] = useState<{ tone: "good" | "bad"; text: string }>()
  const go = async () => {
    const why = await actions.setPassword(name, fresh, old)
    setSaid(why === undefined ? { tone: "good", text: t("own.changed") } : { tone: "bad", text: why })
    setOld("")
    setFresh("")
  }
  return (
    <Field label={t("own.newPassword")} help={t("own.newPasswordHelp")}>
      <TextInput
        label={t("own.current")}
        value={old}
        onChange={setOld}
        placeholder={t("own.currentPlaceholder")}
        secret
      />
      <TextInput
        label={t("own.newPassword")}
        value={fresh}
        onChange={setFresh}
        placeholder={t("own.newPlaceholder")}
        secret
        onEnter={() => void go()}
      />
      <Button onClick={() => void go()} disabled={old === "" || fresh === ""}>
        {t("own.change")}
      </Button>
      {said !== undefined && <Signal tone={said.tone}>{said.text}</Signal>}
    </Field>
  )
}

export function OwnAccount() {
  const { session, can } = useAccess()
  const t = useT("access")
  if (!session.accounts) {
    return (
      <View title={t("own.title")} subtitle={t("own.noAccounts")}>
        <Quiet title={t("own.everyone")} hint={t("own.madeOnMachine")} />
      </View>
    )
  }
  if (session.role === "guest") {
    return (
      <View title={t("own.title")} subtitle={t("own.notSignedIn")}>
        <Link href={signInHome}>{t("own.signIn")}</Link>
      </View>
    )
  }
  if (session.name === null) {
    return (
      <View title={t("own.title")} subtitle={t("own.machine")}>
        <Link href={accountsHome}>{t("own.everyAccount")}</Link>
      </View>
    )
  }
  // Prefilling any game says more than prefilling one's own: only the wider one is listed.
  const ownOnly = can("prefill.own") && !can("prefill")
  const may = permissions.filter(([p]) => can(p) && (p !== "prefill.own" || ownOnly))
  const links = session.links ?? []
  const role = t(`roleName.${session.role}`)
  return (
    <View title={session.name} subtitle={t("own.subtitle", { role })}>
      <Panel title={t("own.may")} note={role}>
        {may.length === 0 ? (
          <Muted>{t("own.readOnly")}</Muted>
        ) : (
          <Stack>
            {may.map(([p, key]) => (
              <Muted key={p}>{t(`permission.${key}`)}</Muted>
            ))}
          </Stack>
        )}
      </Panel>
      {(links.length > 0 || ownOnly) && (
        <Panel title={t("own.links")}>
          {links.length === 0 ? (
            <Muted>{t("own.noLinks")}</Muted>
          ) : (
            <Stack>
              <Tags items={links.map((text) => ({ text, tone: "quiet" as const }))} />
              <Muted>{t("own.linksOwn")}</Muted>
            </Stack>
          )}
        </Panel>
      )}
      <Panel title={t("own.password")}>
        <OwnPassword name={session.name} />
      </Panel>
    </View>
  )
}
