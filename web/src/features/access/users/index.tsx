/**
 * Users: the page of every account (/accounts), for whoever may manage them. The accounts as a
 * roster: each row its avatar and name (yours marked), what its role lets it do, a player's platform
 * accounts as pills to switch on and off (any number of those lanstash is signed in to; shown on
 * another account only while it has some, to take them away), its role
 * switched in place, and a way to remove it. Then adding an account (./add: an invite link, or a
 * password set now), and the invites waiting to be used (./invites).
 */
import { useEffect, useState } from "react"
import type { Invite, User } from "@/contracts/session"
import { useAccess } from "@/features/access"
import { AddAccount } from "@/features/access/add"
import { Invites } from "@/features/access/invites"
import { linkOptions, roleOptions } from "@/features/access/roles"
import { useT } from "@/i18n"
import { Button } from "@/styled/button"
import { Panel } from "@/styled/panel"
import { RoleSwitch, Roster, RosterRow } from "@/styled/roster"
import { Signal } from "@/styled/signal"
import { Stack } from "@/styled/stack"
import { Quiet } from "@/styled/text"
import { Toggles } from "@/styled/toggles"
import { View } from "@/styled/view"

/** `linkChoices`: the platform accounts lanstash is signed in to (`steam/<account>`). */
export function UsersView({ linkChoices }: { readonly linkChoices: readonly string[] }) {
  const { session, can, actions, joinLink } = useAccess()
  const t = useT("access")
  const manages = session.accounts && can("accounts")
  const [list, setList] = useState<readonly User[] | string>()
  const [invites, setInvites] = useState<readonly Invite[] | string>()
  const [reads, setReads] = useState(0)
  const [said, setSaid] = useState<string>()
  useEffect(() => {
    void reads
    if (!manages) return
    let live = true
    void actions.users().then((u) => live && setList(u))
    void actions.invites().then((i) => live && setInvites(i))
    return () => {
      live = false
    }
  }, [actions, manages, reads])
  if (!manages) {
    return (
      <View title={t("users.title")} subtitle={t("users.own")}>
        <Quiet title={session.accounts ? t("users.onlyAdmin") : t("users.noAccounts")} hint={t("users.ownHint")} />
      </View>
    )
  }
  const again = () => setReads((n) => n + 1)
  const after = async (done: Promise<string | undefined>) => {
    setSaid(await done)
    again()
  }
  // Links switched on or off: each change sent in turn, the first refusal said.
  const relink = async (u: User, next: readonly string[]) => {
    const now = u.links ?? []
    let why: string | undefined
    for (const l of next.filter((x) => !now.includes(x))) why = why ?? (await actions.setLink(u.name, l, true))
    for (const l of now.filter((x) => !next.includes(x))) why = why ?? (await actions.setLink(u.name, l, false))
    setSaid(why)
    again()
  }
  const waiting = Array.isArray(invites) ? invites : []
  const count = Array.isArray(list) ? t("users.count", { count: list.length }) : undefined
  const options = roleOptions(t)
  return (
    <View title={t("users.title")} subtitle={t("users.subtitle")} aside={count}>
      <Panel title={t("users.panel")}>
        <Stack wide>
          {typeof list === "string" && <Signal tone="bad">{list}</Signal>}
          {Array.isArray(list) && (
            <Roster label={t("users.list")}>
              {list.map((u: User) => (
                <RosterRow
                  key={u.name}
                  name={u.name}
                  tag={u.name === session.name ? t("users.you") : undefined}
                  note={t(`roleWords.${u.role}`)}
                  role={
                    <RoleSwitch
                      label={t("users.roleOf", { name: u.name })}
                      options={options}
                      value={u.role}
                      onChange={(role) => void after(actions.setRole(u.name, role))}
                    />
                  }
                  actions={
                    <Button
                      tone="danger"
                      onClick={() => void after(actions.removeUser(u.name))}
                      label={t("users.removeName", { name: u.name })}
                    >
                      {t("users.remove")}
                    </Button>
                  }
                >
                  {(u.role === "player" || (u.links ?? []).length > 0) && (
                    <Toggles
                      label={t("users.linksOf", { name: u.name })}
                      options={linkOptions(linkChoices, u.links)}
                      values={u.links ?? []}
                      onChange={(next) => void relink(u, next)}
                      empty={t("users.noLinkYet")}
                    />
                  )}
                </RosterRow>
              ))}
            </Roster>
          )}
          {said !== undefined && <Signal tone="bad">{said}</Signal>}
          {typeof invites === "string" && <Signal tone="bad">{invites}</Signal>}
        </Stack>
      </Panel>
      {waiting.length > 0 && (
        <Panel title={t("users.invites")} note={`${waiting.length}`}>
          <Invites list={waiting} onRevoke={(id) => void after(actions.revokeInvite(id))} />
        </Panel>
      )}
      <Panel title={t("users.add")}>
        <AddAccount actions={actions} joinLink={joinLink} linkChoices={linkChoices} onDone={again} />
      </Panel>
    </View>
  )
}
