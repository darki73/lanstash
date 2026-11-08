/**
 * Games: a platform's games as cards, sorted by size, by name or by last use (the choice is kept
 * while the page is open). Where there are more than a few, a box finds games as one types (by name,
 * id or store code, initials too: "gta"); where some are behind their current build, a choice shows
 * only those; the title then says how many of all are shown. While those are shown, a bar at the
 * bottom offers to queue prefills of them all (those not being prefilled or waiting already, the
 * smallest gap first) and says about how much that downloads. A card shows the game's name (its code when lanstash has no name for
 * it), its version under it where lanstash checks builds, with a badge where the stash does not hold all
 * of that build's install: how much of it (a percentage), or that lanstash is still working it out
 * (checking); its size, its objects (and how many parts, on a platform whose games arrive in
 * parts), a bar for its share of the platform's largest game, when it was last used, and lines for
 * a pre-release build with how much of it is stashed, how much of a build lancache's old cache
 * holds, and one per other product of its directory (Classic Era │ 12 %, its version on hover); with a Prefill button where lanstash can prefill
 * (accented for a game behind, Stop on the game being prefilled) and a Pre-load button for a
 * pre-release build not yet stashed; above them the game's cover art, and beside the name its icon, where the
 * platform has them. The launcher's own data is folded away under the cards. With no games cached, it says how
 * they get there.
 */
import { useState } from "react"
import { type ArtLink, noArt } from "@/contracts/art"
import type { BuildGame } from "@/contracts/builds"
import { type Prefill, running, waiting } from "@/contracts/prefill"
import {
  type Game,
  type GameOrder,
  gameLink,
  type Platform,
  sortGames,
  worthLancache,
} from "@/features/platforms/model"
import { useAgo, useT, type Words } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { match } from "@/lib/match"
import { done } from "@/lib/percent"
import { ActionBar } from "@/styled/actionbar"
import { Button } from "@/styled/button"
import { CardGrid, type CardNote, GameCard } from "@/styled/cards"
import { Fold } from "@/styled/fold"
import { Panel } from "@/styled/panel"
import { Readings } from "@/styled/readings"
import { Search } from "@/styled/search"
import { Segmented } from "@/styled/segmented"
import { Signal } from "@/styled/signal"
import { Muted, Quiet } from "@/styled/text"
import { Toolbar } from "@/styled/toolbar"

const orders = ["size", "name", "used"] as const

/**
 * A game's build as a badge where it needs a look: how much of it is stashed, still being worked out,
 * or a failed lookup. A current one has none (the page's chip says when all are).
 */
function buildBadge(t: Words<"platforms">, b: BuildGame | undefined) {
  if (b === undefined) return undefined
  if (b.version === null) {
    return b.error === null ? undefined : <Signal tone="bad">{t("build.unknown")}</Signal>
  }
  if (b.current) return undefined
  if (b.have == null || b.want == null) {
    return b.plan_error ? (
      <Signal tone="bad" title={b.plan_error}>
        {t("build.unknown")}
      </Signal>
    ) : (
      <Signal tone="waiting">{t("build.checking")}</Signal>
    )
  }
  return (
    <Signal
      tone="waiting"
      still
      title={t("games.stashedOf", { have: size(b.have), want: size(b.want), version: b.version })}
    >
      {done(b.have, b.want)}
    </Signal>
  )
}

/** How much of a build not stashed whole lancache's old cache holds, once probed. */
function inLancache(t: Words<"platforms">, b: BuildGame | undefined): CardNote | undefined {
  const s = b?.lancache_share
  if (s == null || b?.current === true || s.want === 0) return undefined
  return { label: t("games.inLancache"), value: done(s.have, s.want) }
}

/** A line per other product of the game's directory: its name (its version on hover), and how much of it is stashed. */
function flavours(t: Words<"platforms">, b: BuildGame | undefined): CardNote[] {
  return (b?.flavours ?? []).flatMap((f) => {
    const r = f.build
    if (r == null) return []
    const state = r.complete
      ? t("build.current")
      : r.have == null || r.want == null
        ? t("build.checking")
        : done(r.have, r.want)
    const name = f.name ?? f.product
    return [{ label: name, value: state, hint: `${name} ${r.version}` }]
  })
}

/** A game's pre-release build, and how much of it is stashed. */
function prerelease(t: Words<"platforms">, b: BuildGame | undefined): CardNote | undefined {
  const pre = b?.prerelease
  if (pre == null) return undefined
  const stashed = pre.complete
    ? t("build.stashed")
    : pre.have == null || pre.want == null
      ? t("build.checking")
      : done(pre.have, pre.want)
  return { label: t("games.prerelease", { version: pre.version }), value: stashed }
}

/** A game's objects, and its parts when it has several. */
function detail(t: Words<"platforms">, p: Platform, parts: number, objects: number): string {
  const all = t("games.objects", { n: count(objects) })
  return p.part === undefined || parts === 1 ? all : t(`games.${p.part}s`, { count: parts, objects: all })
}

export interface GamesProps {
  readonly platform: Platform
  /** Where a game's cover image is. */
  readonly art?: ArtLink
  /** The time now (ms), for how long ago a game was used. */
  readonly now: number
  /** Prefill, on a platform that has it: the last job, and how to start and stop one. */
  readonly prefill?: GamesPrefill
}

export interface GamesPrefill {
  readonly job: Prefill | null | undefined
  /** Starts a game's current build, or its pre-release build (from lancache's old cache first when asked). */
  readonly start: (game: string, prerelease: boolean, fromLancache?: boolean) => void
  /** Stops the running prefill; none: the account may start prefills but not stop them. */
  readonly stop?: () => void
  /** Which games may be prefilled (a player's own); none: every game. */
  readonly may?: (game: string) => boolean
  /** Whether lanstash can take builds from lancache's old cache. */
  readonly lancache?: boolean
  /** Queues prefills of `games`, in this order (lancache's old cache first where lanstash can read it). */
  readonly queueAll?: (games: readonly string[]) => void
}

/** What bringing `games` up to date takes: the bytes missing of those worked out, and how many are not yet. */
function missing(games: readonly Game[]) {
  let bytes = 0
  let unknown = 0
  for (const g of games) {
    const b = g.build
    if (b?.have == null || b.want == null) unknown += 1
    else bytes += Math.max(0, b.want - b.have)
  }
  return { bytes, unknown }
}

/**
 * A game's prefill buttons: Prefill (from the CDN), From lancache (lancache's old cache first, the
 * CDN for the rest; where lanstash can read it, probed it for the build, and it holds more of the
 * build than the stash: `worthLancache`), and Pre-load for
 * a pre-release build not yet stashed; Stop on the game being prefilled. While the platform prefills
 * another game, the same choice queues this one after it: Queue from lancache (lancache's old cache
 * first) and Queue (the CDN only); a game waiting in the queue says so.
 */
function prefillButton(
  t: Words<"platforms">,
  name: string,
  code: string,
  current: BuildGame | undefined,
  pf: GamesPrefill,
) {
  if (current?.version == null || pf.may?.(code) === false) return undefined
  const pre = current.prerelease
  const fromLancache = !current.current && worthLancache(pf.lancache, current.lancache_share, current.have)
  if (running(pf.job) && pf.job.game === code) {
    if (pf.stop === undefined) return undefined
    return (
      <Button onClick={pf.stop} label={t("button.stopLabel", { name })}>
        {t("button.stop")}
      </Button>
    )
  }
  if (waiting(pf.job, code)) {
    return (
      <Button onClick={() => {}} disabled label={t("button.queuedLabel", { name })}>
        {t("button.queued")}
      </Button>
    )
  }
  if (running(pf.job)) {
    // The same choice as when nothing runs: lancache's old cache first, or the internet only.
    return (
      <>
        {fromLancache && (
          <Button onClick={() => pf.start(code, false, true)} label={t("button.queueAfterFromLancacheLabel", { name })}>
            {t("button.queueFromLancache")}
          </Button>
        )}
        <Button onClick={() => pf.start(code, false)} label={t("button.queueLabel", { name })}>
          {t("button.queue")}
        </Button>
      </>
    )
  }
  return (
    <>
      {pre != null && !pre.complete && (
        <Button onClick={() => pf.start(code, true)} label={t("button.preloadLabel", { name, version: pre.version })}>
          {t("button.preload")}
        </Button>
      )}
      {fromLancache && (
        <Button onClick={() => pf.start(code, false, true)} label={t("button.prefillFromLancacheLabel", { name })}>
          {t("button.fromLancache")}
        </Button>
      )}
      <Button
        onClick={() => pf.start(code, false)}
        tone={current.current ? "plain" : "accent"}
        label={t("button.prefillLabel", { name })}
      >
        {t("button.prefill")}
      </Button>
    </>
  )
}

/** Whether a game's build is known and the stash does not hold all of it. */
const behind = (g: Game) => g.build?.version != null && !g.build.current

/** How well a game answers what was typed: by its name, its id or a store code; none: it does not. */
function score(g: Game, typed: string): number | undefined {
  const all = [g.name, g.id ?? "", ...g.codes].map((n) => match(n, typed)).filter((s) => s !== undefined)
  return all.length === 0 ? undefined : Math.max(...all)
}

export function Games({ platform: p, art = noArt, now, prefill }: GamesProps) {
  const ago = useAgo()
  const t = useT("platforms")
  const tCommon = useT("common")
  const [order, setOrder] = useState<GameOrder>("size")
  const [typed, setTyped] = useState("")
  const [onlyBehind, setOnlyBehind] = useState(false)
  const largest = Math.max(1, ...p.games.map((g) => g.bytes))
  const lagging = p.games.filter(behind).length
  const shown = sortGames(p.games, order).filter(
    (g) => score(g, typed) !== undefined && (!onlyBehind || lagging === 0 || behind(g)),
  )
  const nameOf = (g: Game) => (g.privacy === "masked" ? tCommon("hiddenGame") : g.name)
  const filtering = shown.length !== p.games.length
  // The behind games shown that are not already being prefilled or waiting, the smallest gap first.
  const busy = (g: Game) =>
    g.id === undefined ||
    prefill?.may?.(g.id) === false ||
    (running(prefill?.job) && prefill.job.game === g.id) ||
    waiting(prefill?.job, g.id)
  const toQueue = onlyBehind
    ? shown.filter((g) => behind(g) && !busy(g)).sort((a, b) => missing([a]).bytes - missing([b]).bytes)
    : []
  const gap = missing(toQueue)
  return (
    <>
      <Toolbar
        title={
          filtering
            ? t("games.shown", { count: p.games.length, shown: count(shown.length), n: count(p.games.length) })
            : t("games.inCache", { count: p.games.length, n: count(p.games.length) })
        }
        tools={
          <>
            {p.games.length > 6 && (
              <Search
                label={t("games.find", { name: p.name })}
                value={typed}
                onChange={setTyped}
                placeholder={t("games.findPlaceholder")}
                compact
              />
            )}
            {lagging > 0 && (
              <Segmented
                label={t("games.which")}
                options={[
                  { id: "all", label: t("games.all") },
                  { id: "behind", label: t("games.behind", { n: count(lagging) }) },
                ]}
                value={onlyBehind ? "behind" : "all"}
                onChange={(v) => setOnlyBehind(v === "behind")}
              />
            )}
            <Segmented
              label={t("games.sort")}
              options={orders.map((id) => ({ id, label: t(`games.order.${id}`) }))}
              value={order}
              onChange={setOrder}
            />
          </>
        }
      />
      {p.games.length > 0 && shown.length === 0 && (
        <Quiet title={t("games.noMatch.title", { typed })} hint={t("games.noMatch.hint")} />
      )}
      {p.games.length === 0 ? (
        <Panel title={t("games.title")}>
          <Quiet title={t("games.empty.title", { name: p.name })} hint={t("games.empty.hint", { name: p.name })} />
        </Panel>
      ) : (
        <CardGrid>
          {shown.map((g) => (
            <GameCard
              key={g.key}
              name={nameOf(g)}
              href={g.id === undefined || g.privacy === "masked" ? undefined : gameLink(p.id, g.id)}
              note={g.privacy === "revealed" ? t("games.hidden") : g.named ? undefined : t("games.unnamed")}
              image={g.art === undefined ? undefined : art(p.id, g.art)}
              cover={p.art?.cover}
              icon={g.art !== undefined && p.art?.icon ? art(p.id, g.art, "icon") : undefined}
              badge={buildBadge(t, g.build)}
              size={size(g.bytes)}
              detail={detail(t, p, g.codes.length, g.objects)}
              sub={g.build?.version ? t("games.version", { version: g.build.version }) : undefined}
              foot={[t("games.used", { ago: ago(now, g.lastUsedMs) })]}
              notes={[prerelease(t, g.build), inLancache(t, g.build), ...flavours(t, g.build)].filter(
                (n) => n !== undefined,
              )}
              action={
                p.prefill && prefill !== undefined && g.id !== undefined
                  ? prefillButton(t, nameOf(g), g.id, g.build, prefill)
                  : undefined
              }
              fraction={g.bytes / largest}
              tone="cache"
            />
          ))}
        </CardGrid>
      )}
      {toQueue.length > 0 && prefill?.queueAll !== undefined && (
        <ActionBar
          label={t("games.catchUp.label")}
          actions={
            <Button
              onClick={() => prefill.queueAll?.(toQueue.flatMap((g) => (g.id === undefined ? [] : [g.id])))}
              tone="accent"
              label={t("games.catchUp.queueLabel", { count: toQueue.length, n: count(toQueue.length) })}
            >
              {t("button.queuePrefills", { count: toQueue.length, n: count(toQueue.length) })}
            </Button>
          }
        >
          <Muted>
            {t("games.catchUp.gap", { count: toQueue.length, n: count(toQueue.length), size: size(gap.bytes) })}
            {gap.unknown > 0 ? t("games.catchUp.unknown", { n: count(gap.unknown) }) : ""}
            {prefill.lancache === true ? t("games.catchUp.lancache") : ""}
          </Muted>
          <Muted>{t("games.catchUp.order")}</Muted>
        </ActionBar>
      )}
      {p.systemData.length > 0 && (
        <Fold
          title={t("games.launcherData")}
          summary={`${p.systemData.map((s) => s.game).join(" · ")} — ${size(p.systemData.reduce((a, s) => a + s.bytes, 0))}`}
        >
          <Readings
            rows={p.systemData.map((s) => ({
              name: s.game,
              value: size(s.bytes),
              note: t("games.objectsShort", { n: count(s.objects) }),
            }))}
          />
        </Fold>
      )}
    </>
  )
}
