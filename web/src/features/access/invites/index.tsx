/**
 * Invites: the invites not used yet, on the accounts page: each a row with its account's name when
 * the admin chose it, when it runs out, its platform accounts and its role, and a way to take it
 * back. Nothing is shown while there are none.
 */
import type { Invite } from "@/contracts/session"
import { toneOf } from "@/features/access/roles"
import { useLanguage, useT } from "@/i18n"
import { Button } from "@/styled/button"
import { RoleBadge, Roster, RosterRow } from "@/styled/roster"
import { Tags } from "@/styled/tags"

/** When an invite runs out, in `language`'s words and the reader's time ("Mon 15 Nov, 14:05"). */
export const until = (ms: number, language: string) =>
  new Date(ms).toLocaleString(language, {
    weekday: "short",
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  })

export function Invites({
  list,
  onRevoke,
}: {
  readonly list: readonly Invite[]
  readonly onRevoke: (id: string) => void
}) {
  const t = useT("access")
  const { code } = useLanguage()
  if (list.length === 0) return null
  return (
    <Roster label={t("invites.list")}>
      {list.map((i) => {
        const end = until(i.untilMs, code)
        return (
          <RosterRow
            key={i.id}
            name={i.name ?? t("invites.anyone")}
            tag={t("invites.tag")}
            mark="✉"
            note={i.name === null ? t("invites.theyChoose", { until: end }) : t("invites.worksUntil", { until: end })}
            role={<RoleBadge label={t(`roleName.${i.role}`)} tone={toneOf(i.role)} />}
            actions={
              <Button tone="danger" onClick={() => onRevoke(i.id)} label={t("invites.takeBackId", { id: i.id })}>
                {t("invites.takeBack")}
              </Button>
            }
          >
            {i.links.length > 0 && <Tags items={i.links.map((text) => ({ text, tone: "quiet" as const }))} />}
          </RosterRow>
        )
      })}
    </Roster>
  )
}
