/**
 * Add: one way to add an account, on the accounts page. The admin chooses its role (what it may do
 * written under it) and, for a player, any number of the platform accounts lanstash is signed in
 * to, then who sets its password:
 *   - they do: lanstash makes an invite link (shown once, to copy and send) that works once, for a
 *     week; the name is the admin's choice, or, left empty, theirs;
 *   - the admin, now: the account is made at once with the name and password typed here.
 * `onDone` follows every change (the lists are read again).
 */
import { useState } from "react"
import type { SessionActions } from "@/contracts/session"
import { until } from "@/features/access/invites"
import { linkOptions, type Role, roleOptions } from "@/features/access/roles"
import { useLanguage, useT } from "@/i18n"
import { Button } from "@/styled/button"
import { Field, TextInput } from "@/styled/field"
import { RoleSwitch } from "@/styled/roster"
import { Segmented } from "@/styled/segmented"
import { Signal } from "@/styled/signal"
import { Stack } from "@/styled/stack"
import { Command } from "@/styled/steps"
import { Muted } from "@/styled/text"
import { Toggles } from "@/styled/toggles"

type Who = "they" | "now"

export interface AddProps {
  readonly actions: SessionActions
  readonly joinLink: (token: string) => string
  /** The platform accounts lanstash is signed in to. */
  readonly linkChoices: readonly string[]
  readonly onDone: () => void
}

export function AddAccount({ actions, joinLink, linkChoices, onDone }: AddProps) {
  const t = useT("access")
  const { code } = useLanguage()
  const [who, setWho] = useState<Who>("they")
  const [name, setName] = useState("")
  const [role, setRole] = useState<Role>("player")
  const [chosen, setLinks] = useState<readonly string[]>([])
  const [password, setPassword] = useState("")
  const [result, setResult] = useState<
    { kind: "link"; link: string; untilMs: number } | { kind: "added"; name: string } | { kind: "bad"; why: string }
  >()
  const [busy, setBusy] = useState(false)
  const ready = !busy && (who === "they" || (name.trim() !== "" && password !== ""))
  const reset = () => {
    setName("")
    setPassword("")
    setLinks([])
  }
  const go = async () => {
    setBusy(true)
    const n = name.trim()
    // Platform accounts matter only to a player.
    const links = role === "player" ? chosen : []
    if (who === "they") {
      const r = await actions.makeInvite(role, links, n)
      if (typeof r === "string") setResult({ kind: "bad", why: r })
      else {
        setResult({ kind: "link", link: joinLink(r.token), untilMs: r.untilMs })
        reset()
      }
    } else {
      let why = await actions.addUser(n, role, password)
      for (const l of links) why = why ?? (await actions.setLink(n, l, true))
      setResult(why === undefined ? { kind: "added", name: n } : { kind: "bad", why })
      if (why === undefined) reset()
    }
    setBusy(false)
    onDone()
  }
  const whoOptions: readonly { readonly id: Who; readonly label: string }[] = [
    { id: "they", label: t("add.they") },
    { id: "now", label: t("add.now") },
  ]
  return (
    <>
      <Field label={t("add.password")} help={t("add.passwordHelp")}>
        <Segmented label={t("add.who")} options={whoOptions} value={who} onChange={setWho} />
      </Field>
      <Field label={t("add.name")} help={who === "they" ? t("add.nameHelpThey") : t("add.nameHelpNow")}>
        <TextInput
          label={t("add.nameLabel")}
          value={name}
          placeholder={who === "they" ? t("add.namePlaceholderThey") : t("add.namePlaceholderNow")}
          onChange={setName}
        />
      </Field>
      <Field label={t("add.role")} help={t(`roleWords.${role}`)}>
        <RoleSwitch label={t("add.roleLabel")} options={roleOptions(t)} value={role} onChange={setRole} />
      </Field>
      {role === "player" && (
        <Field label={t("add.links")} help={t("add.linksHelp")}>
          <Toggles
            label={t("add.linksLabel")}
            options={linkOptions(linkChoices)}
            values={chosen}
            onChange={setLinks}
            empty={t("add.noLinks")}
          />
        </Field>
      )}
      {who === "now" && (
        <Field label={t("add.itsPassword")} help={t("add.itsPasswordHelp")}>
          <TextInput
            label={t("add.passwordLabel")}
            value={password}
            placeholder={t("add.passwordPlaceholder")}
            secret
            onChange={setPassword}
            onEnter={() => ready && void go()}
          />
        </Field>
      )}
      <Field
        label={who === "they" ? t("add.inviteLink") : t("add.addIt")}
        help={who === "they" ? t("add.inviteLinkHelp") : t("add.addItHelp")}
      >
        <Button tone="accent" disabled={!ready} onClick={() => void go()}>
          {who === "they" ? t("add.makeLink") : t("add.addAccount")}
        </Button>
      </Field>
      {result !== undefined && (
        <Stack>
          {result.kind === "link" && <Command label={t("add.theLink")} text={result.link} />}
          {result.kind === "link" && <Muted>{t("add.worksUntil", { until: until(result.untilMs, code) })}</Muted>}
          {result.kind === "added" && <Signal tone="good">{t("add.added", { name: result.name })}</Signal>}
          {result.kind === "bad" && <Signal tone="bad">{result.why}</Signal>}
        </Stack>
      )}
    </>
  )
}
