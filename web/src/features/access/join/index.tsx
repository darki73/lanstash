/**
 * Join: the page an invite's link opens (/join/<token>). It first asks lanstash what the invite
 * makes: the role (said under the title) and, when the admin chose it, the account's name (shown,
 * not to be changed). Whoever holds the link then chooses a password (twice, so a slip of the keys
 * does not lock them out) and a name if none was chosen; lanstash makes the account with the role
 * and platform accounts the admin chose, uses the invite up, and signs it in. An invite used, taken
 * back or run out says so; a signed-in page is told so, with the way on.
 */
import { useEffect, useState } from "react"
import type { InviteView } from "@/contracts/session"
import { SignedIn, signInHome, useAccess } from "@/features/access"
import { useT } from "@/i18n"
import { Gate } from "@/styled/gate"
import { Link, Quiet } from "@/styled/text"
import { View } from "@/styled/view"

export function Join({ token, done }: { readonly token: string; readonly done: string }) {
  const { session, actions } = useAccess()
  const t = useT("access")
  const [invite, setInvite] = useState<InviteView | string>()
  const [name, setName] = useState("")
  const [password, setPassword] = useState("")
  const [again, setAgain] = useState("")
  const [refused, setRefused] = useState<string>()
  const [busy, setBusy] = useState(false)
  const guest = session.role === "guest"
  useEffect(() => {
    if (!guest) return
    let live = true
    void actions.invite(token).then((i) => {
      if (!live) return
      setInvite(i)
      if (typeof i !== "string" && i.name !== null) setName(i.name)
    })
    return () => {
      live = false
    }
  }, [actions, token, guest])
  if (!guest) return <SignedIn done={done} />
  if (typeof invite === "string") {
    return (
      <View title={t("join.title")} subtitle={t("join.invalid")}>
        <Quiet title={invite} hint={t("join.askNew")} />
        <Link href={signInHome}>{t("join.signInInstead")}</Link>
      </View>
    )
  }
  const go = async () => {
    if (password !== again) {
      setRefused(t("join.differ"))
      return
    }
    setBusy(true)
    const r = await actions.join(token, name.trim(), password)
    setBusy(false)
    // Joined, the session changes and this page says so, with the way on.
    if ("error" in r) setRefused(r.error)
  }
  const fixed = invite?.name != null
  const subtitle =
    invite === undefined
      ? t("join.invited")
      : t(fixed ? "join.invitedNamed" : "join.invitedFree", { role: t(`join.asRole.${invite.role}`) })
  return (
    <Gate
      title={t("join.title")}
      subtitle={subtitle}
      fields={[
        { label: t("join.name"), value: name, onChange: setName, kind: "username", fixed },
        { label: t("join.password"), value: password, onChange: setPassword, kind: "new-password" },
        { label: t("join.again"), value: again, onChange: setAgain, kind: "new-password" },
      ]}
      submit={busy ? t("join.busy") : t("join.submit")}
      onSubmit={() => void go()}
      busy={busy || invite === undefined}
      error={refused}
      hint={fixed ? t("join.hintNamed") : t("join.hintFree")}
    />
  )
}
