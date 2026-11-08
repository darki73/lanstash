/**
 * History: what was updated when, newest first, grouped by day (in the viewer's time zone). Each
 * event names its game (by the stash's name for it, else the catalog's, else its id), links to the
 * game's page, says its platform, and what happened: a new build (the old one struck through before
 * it), the stash coming to hold all of a build (current), a game seen for the first time (added).
 * lanstash notes these as it works the builds out, so the page follows the status stream.
 */
import type { CatalogByPlatform } from "@/contracts/catalog"
import type { Privacy } from "@/contracts/hidden"
import type { History, HistoryEvent } from "@/contracts/history"
import type { Snapshot } from "@/contracts/status"
import { gameLink, owns, type Platform, platforms } from "@/features/platforms/model"
import { useT, type Words } from "@/i18n"
import { Brand } from "@/styled/brand"
import { Monogram } from "@/styled/monogram"
import { Panel } from "@/styled/panel"
import { Quiet } from "@/styled/text"
import { Timeline, type TimelineDay, type TimelineEntry, type TimelineKind } from "@/styled/timeline"
import { View } from "@/styled/view"

export interface HistoryViewProps {
  readonly history: History
  readonly snapshot: Snapshot
  readonly catalogs?: CatalogByPlatform
  /** How a game shows (a hidden one masked: its name, page and builds would tell it); none: plainly. */
  readonly privacy?: (platform: string, game: string) => Privacy
}

const kinds: Readonly<
  Record<HistoryEvent["change"], { readonly kind: TimelineKind; readonly label: "newBuild" | "current" | "added" }>
> = {
  "new-build": { kind: "new", label: "newBuild" },
  current: { kind: "done", label: "current" },
  added: { kind: "seen", label: "added" },
}

const pad = (n: number) => String(n).padStart(2, "0")

/** The time of day of `ms`, in the viewer's time zone ("14:05"). */
export const timeOf = (ms: number) => {
  const d = new Date(ms)
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`
}

/** The day of `ms` as a heading: Today, Yesterday, else "Mon 3 Nov" (with the year when not this one's). */
export function dayOf(ms: number, now: number, t: Words<"history">): string {
  const d = new Date(ms)
  const start = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime()
  const today = start(new Date(now))
  const day = start(d)
  if (day === today) return t("day.today")
  // A day back from today's start: Yesterday (calendar days, whatever the clock change).
  if (day === start(new Date(today - 12 * 3_600_000))) return t("day.yesterday")
  const parts = {
    weekday: t(`weekday.${weekdays[d.getDay()] ?? "sun"}`),
    day: d.getDate(),
    month: t(`month.${months[d.getMonth()] ?? "jan"}`),
  }
  return d.getFullYear() === new Date(now).getFullYear()
    ? t("day.date", parts)
    : t("day.dateYear", { ...parts, year: d.getFullYear() })
}

const weekdays = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"] as const
const months = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"] as const

/** An event's game: its name and its page, as the stash or the catalog know it. */
function gameOf(p: Platform | undefined, catalogs: CatalogByPlatform | undefined, e: HistoryEvent) {
  const cached = p?.games.find((g) => owns(g, e.game))
  const listed = catalogs?.[e.platform]?.products.find((c) => c.game === e.game || c.code === e.game)
  const name = (cached?.named ? cached.name : undefined) ?? listed?.name ?? cached?.name ?? e.game
  const page = cached?.id ?? listed?.game ?? e.game
  return { name, href: gameLink(e.platform, page) }
}

/** An event, and the "current" that finished it when one did (newest first, as the history lists them). */
interface Step {
  readonly event: HistoryEvent
  readonly finished?: HistoryEvent
}

/**
 * The events with each "current" folded into the new build or first sighting just before it of the
 * same game and build: one line says the build came and the stash then held all of it.
 */
export function steps(events: readonly HistoryEvent[]): Step[] {
  const out: Step[] = []
  for (let i = 0; i < events.length; i++) {
    const e = events[i] as HistoryEvent
    const before = events[i + 1]
    const finishes =
      e.change === "current" &&
      before !== undefined &&
      before.change !== "current" &&
      before.platform === e.platform &&
      before.game === e.game &&
      before.version === e.version
    if (finishes) {
      out.push({ event: before, finished: e })
      i++
    } else out.push({ event: e })
  }
  return out
}

/** The events as days of entries, newest first; `hidden` is what a masked game is called. */
export function days(props: HistoryViewProps, t: Words<"history">, hidden: string): TimelineDay[] {
  const { history, snapshot, catalogs, privacy } = props
  const known = platforms(snapshot.usage)
  const out: { label: string; entries: TimelineEntry[] }[] = []
  steps(history.events).forEach(({ event: e, finished }, i) => {
    const p = known.find((x) => x.id === e.platform)
    const masked = e.hidden === true || privacy?.(e.platform, e.game) === "masked"
    const game = masked ? { name: hidden, href: undefined } : gameOf(p, catalogs, e)
    const first = kinds[e.change]
    // Finished: the line stands where the stash came to hold it, and says what came first.
    const at = finished?.at_ms ?? e.at_ms
    const kind = finished === undefined ? first.kind : kinds.current.kind
    const label =
      finished === undefined
        ? t(`change.${first.label}`)
        : t("change.thenCurrent", { change: t(`change.${first.label}`) })
    const entry: TimelineEntry = {
      key: `${i}-${at}-${e.platform}-${e.game}-${e.change}`,
      time: timeOf(at),
      icon: p?.brand !== undefined ? <Brand name={p.brand} /> : <Monogram letters={p?.letters ?? "?"} />,
      title: game.name,
      ...(game.href === undefined ? {} : { href: game.href }),
      subtitle: p?.name ?? e.platform,
      kind,
      label,
      // A build number would tell which game it is.
      detail: masked ? "" : e.version,
      ...(e.from !== null && !masked && { before: e.from }),
    }
    const day = dayOf(at, snapshot.now_ms, t)
    const last = out[out.length - 1]
    if (last?.label === day) last.entries.push(entry)
    else out.push({ label: day, entries: [entry] })
  })
  return out
}

export function HistoryView(props: HistoryViewProps) {
  const t = useT("history")
  const tCommon = useT("common")
  const grouped = days(props, t, tCommon("hiddenGame"))
  return (
    <View title={t("title")} subtitle={t("subtitle")}>
      <Panel title={t("updates")}>
        {grouped.length === 0 ? (
          <Quiet title={t("none.title")} hint={t("none.hint")} />
        ) : (
          <Timeline label={t("updates")} days={grouped} />
        )}
      </Panel>
    </View>
  )
}
