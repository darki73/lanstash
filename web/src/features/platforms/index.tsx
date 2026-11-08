/**
 * Platforms: a platform's page (the sidebar's links are `features/platforms/nav`). Every platform's
 * page is laid out the same way, from its description: the
 * title with the mark and a line about the platform, chips with its figures (stored, share of the
 * cache, games, objects, downloads in flight, and where lanstash checks builds, how many games have
 * an update to fetch), then its parts, each with a link in the page's own navigation:
 *   - Cached: its games as cards (with their cover art where lanstash serves it, each leading to its
 *     own page), the launcher's own data folded under them; above them, a line about the running
 *     prefill job, leading to it;
 *   - Library (where the platform has a catalog and lanstash can prefill): the games not in the
 *     cache, to choose and queue, with what that takes;
 *   - Prefill (where lanstash can prefill): the platform's own job, its progress or how it ended,
 *     and the games queued after it;
 *   - Account (where lanstash signs in to one, Steam): the account and its QR sign-in; its link
 *     is marked when lanstash is not signed in, or the sign-in failed;
 *   - Activity: what is downloading for it right now, a deletion of one of its games, and how its
 *     content reaches the cache;
 *   - Setup, last (where a PC needs setting up before lanstash keeps the downloads, Epic Games):
 *     how, per operating system.
 * Every prefill started here queues: it runs at once when the platform is idle, else after the
 * running job. Why a start was refused is shown above the part.
 */
import { type ReactNode, useState } from "react"
import { type AccountActions, type AccountState, needsAttention } from "@/contracts/account"
import { type ArtLink, noArt } from "@/contracts/art"
import type { Builds } from "@/contracts/builds"
import { type Catalog, notCached } from "@/contracts/catalog"
import { noPrefill, type PrefillActions, running } from "@/contracts/prefill"
import type { RunsReader } from "@/contracts/runs"
import { noSetup, type SetupLink } from "@/contracts/setup"
import { recentDeletion } from "@/contracts/stash"
import type { Snapshot } from "@/contracts/status"
import { useAccess } from "@/features/access"
import { AccountPanel } from "@/features/platforms/account"
import { DeletingPanel } from "@/features/platforms/deleting"
import { Games } from "@/features/platforms/games"
import {
  downloadsOf,
  gameLink,
  jobOf,
  owns,
  type Platform,
  platform,
  type Section,
  sectionLink,
  worthLancache,
} from "@/features/platforms/model"
import { Offers } from "@/features/platforms/offers"
import { JobLine, PrefillPanel } from "@/features/platforms/prefill"
import { RunsPanel } from "@/features/platforms/runs"
import { SetupPanel } from "@/features/platforms/setup"
import { usePrivacy } from "@/features/privacy"
import { DownloadList } from "@/features/status/downloads/list"
import { storage } from "@/features/status/model"
import { useT, type Words } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { Brand } from "@/styled/brand"
import { Chip, Chips } from "@/styled/chips"
import { Monogram } from "@/styled/monogram"
import { Notes } from "@/styled/notes"
import { Panel } from "@/styled/panel"
import { SectionLink, Sections } from "@/styled/sections"
import { Signal } from "@/styled/signal"
import { Quiet } from "@/styled/text"
import { View } from "@/styled/view"

/** Where builds are behind: in a CDN region (EU), or for an operating system (Windows). */
export const behindLabel = (t: Words<"platforms">, scope: "region" | "os", where: string) =>
  scope === "region"
    ? t("chip.behindIn", { where: where.toUpperCase() })
    : t("chip.behindFor", { where: `${where.charAt(0).toUpperCase()}${where.slice(1)}` })

/** A share in percent: one decimal below 10 %, whole above. */
const percent = (share: number) => `${(100 * share).toFixed(share > 0 && share < 0.1 ? 1 : 0)} %`

/** The parts a platform's page has, in order. */
export const partsOf = (p: Platform): readonly Section[] => [
  "cached",
  ...(p.catalog ? (["library"] as const) : []),
  ...(p.prefill ? (["prefill"] as const) : []),
  ...(p.account ? (["account"] as const) : []),
  "activity",
  ...(p.setup ? (["setup"] as const) : []),
]

export interface PlatformViewProps {
  readonly snapshot: Snapshot
  readonly id: string
  /** The part of the page shown (the cached games when not said, or when the platform has no such part). */
  readonly section?: Section
  /** Where a game's cover image is. */
  readonly art?: ArtLink
  /** The builds lanstash checked, once the API said them. */
  readonly builds?: Builds
  /** Starts and stops prefills. */
  readonly prefill?: PrefillActions
  /** Every game of the platform there is (Battle.net), once the API said them. */
  readonly catalog?: Catalog
  /** The platform's accounts (Steam's, Epic's), once the API said them; null where lanstash offers none. */
  readonly account?: AccountState | null
  /** Signs in to and out of the account. */
  readonly accountActions?: AccountActions
  /** The last snapshots, oldest first: the running job's pace. */
  readonly history?: readonly Snapshot[]
  /** Where the file that sets up a PC for the platform is. */
  readonly setup?: SetupLink
  /** The prefill jobs that ended, as lanstash kept them; none: no past prefills shown. */
  readonly runs?: RunsReader
}

export function PlatformView({
  snapshot,
  id,
  section = "cached",
  art = noArt,
  builds,
  prefill = noPrefill,
  catalog,
  account,
  accountActions,
  history = [],
  setup = noSetup,
  runs,
}: PlatformViewProps) {
  const [refused, setRefused] = useState<string>()
  const t = useT("platforms")
  const tCommon = useT("common")
  const privacy = usePrivacy()
  const { can } = useAccess()
  const p = platform(snapshot.usage, id, builds)
  if (p === undefined) {
    return <Quiet title={t("noPlatform.title", { id })} hint={t("noPlatform.hint")} />
  }
  const total = snapshot.usage.reduce((a, u) => a + u.bytes, 0)
  const downloads = downloadsOf(snapshot.downloads, p.id)
  const inFlight = new Set(downloads.map((d) => d.key)).size
  const recent = recentDeletion(snapshot.deleting, snapshot.now_ms)
  const deleting = recent?.platform === p.id ? recent : undefined
  const job = jobOf(snapshot, p.id)
  const offers = p.catalog
    ? notCached(
        catalog,
        new Set([
          ...p.games.flatMap((g) => (g.id === undefined ? g.codes : [g.id, ...g.codes])),
          ...p.systemData.map((s) => s.game),
        ]),
      )
    : []
  const masked = (game: string) =>
    privacy.of(p.id, game) === "masked" || p.games.find((g) => owns(g, game))?.privacy === "masked"
  const nameOf = (game: string) =>
    masked(game)
      ? tCommon("hiddenGame")
      : (p.games.find((g) => owns(g, game))?.name ?? catalog?.products.find((o) => o.game === game)?.name ?? game)
  const product = (game: string) => catalog?.products.find((o) => o.game === game)
  const artOf = (game: string) => {
    if (masked(game)) return undefined
    const g = p.games.find((x) => owns(x, game))
    if (g?.art !== undefined) return art(p.id, g.art)
    const listed = product(game)
    if (p.art === undefined) return undefined
    return art(p.id, p.art.by === "group" ? game : (listed?.code ?? game))
  }
  // What a prefill downloads: what a cached game's build lacks, or a game's whole download.
  const sizeOf = (game: string) => {
    const b = p.games.find((x) => owns(x, game))?.build
    if (b?.have != null && b.want != null) return Math.max(0, b.want - b.have)
    return product(game)?.download
  }
  const samples = history.map((s) => ({ at: s.now_ms, job: jobOf(s, p.id) }))
  const start = (game: string, prerelease: boolean, from?: boolean | "only") => {
    setRefused(undefined)
    void prefill.start(p.id, game, prerelease, from, true).then(setRefused)
  }
  // Behind: a build whose install is known not to be all stashed (not one still being worked out).
  const behind = p.games.filter((g) => g.build?.version && !g.build.current && g.build.want !== null).length
  const signedOut = needsAttention(account?.view)
  const figures: Readonly<Record<Section, string | undefined>> = {
    cached: count(p.games.length),
    library: catalog === undefined ? undefined : count(offers.length),
    prefill:
      running(job) && job.files_total > 0 ? `${Math.floor((100 * job.files_done) / job.files_total)} %` : undefined,
    account: undefined,
    setup: undefined,
    activity: inFlight > 0 ? count(inFlight) : undefined,
  }
  const dots = {
    prefill: running(job) ? "busy" : undefined,
    account: signedOut ? "attention" : undefined,
    activity: inFlight > 0 ? "busy" : undefined,
  } as const
  /** Whether a prefill of `game` takes lancache's old cache first: only where it holds more of it than the stash. */
  const lancacheFirst = (game: string) => {
    const b = builds?.games.find((g) => g.game === game)
    const share = b?.lancache_share ?? catalog?.products.find((c) => c.game === game)?.lancache
    return b?.current !== true && worthLancache(builds?.lancache, share, b?.have)
  }
  /** Queues prefills of `games` one after another, so they queue in this order; the first refusal is shown. */
  const queueMany = (games: readonly string[], from: "lancache-only" | "prefill") => {
    setRefused(undefined)
    void games
      .reduce<Promise<string | undefined>>(
        (prev, game) =>
          prev.then(
            (why) =>
              why ?? prefill.start(p.id, game, false, from === "lancache-only" ? "only" : lancacheFirst(game), true),
          ),
        Promise.resolve(undefined),
      )
      .then(setRefused)
  }
  const parts: Readonly<Record<Section, () => ReactNode>> = {
    cached: () => (
      <>
        {running(job) && (
          <JobLine
            job={job}
            name={nameOf(job.game)}
            image={artOf(job.game)}
            now={snapshot.now_ms}
            href={sectionLink(p.id, "prefill")}
          />
        )}
        <Games
          platform={p}
          art={art}
          now={snapshot.now_ms}
          prefill={
            can("prefill")
              ? {
                  job,
                  start,
                  stop: () => void prefill.stop(p.id),
                  lancache: builds?.lancache,
                  queueAll: (games) => queueMany(games, "prefill"),
                }
              : can("prefill.own") && p.prefill
                ? {
                    // A player prefills the games its linked accounts own: those its catalog lists.
                    job,
                    start,
                    may: (game) => product(game) !== undefined,
                    lancache: builds?.lancache,
                    queueAll: (games) => queueMany(games, "prefill"),
                  }
                : undefined
          }
        />
      </>
    ),
    library: () => (
      <Offers
        platform={p}
        offers={offers}
        art={art}
        free={storage(snapshot).free}
        lancache={builds?.lancache}
        queue={p.prefill && (can("prefill") || can("prefill.own")) ? queueMany : undefined}
      />
    ),
    prefill: () => (
      <>
        <PrefillPanel
          job={job}
          name={job ? nameOf(job.game) : ""}
          nameOf={nameOf}
          artOf={artOf}
          sizeOf={sizeOf}
          linkOf={(game) => gameLink(p.id, game)}
          samples={samples}
          now={snapshot.now_ms}
          onStop={can("prefill") ? () => void prefill.stop(p.id) : undefined}
          onUnqueue={
            can("prefill")
              ? (game) => {
                  setRefused(undefined)
                  void prefill.unqueue(p.id, game).then(setRefused)
                }
              : undefined
          }
          idle={t(p.catalog ? "prefill.idleWithLibrary" : "prefill.idle", { name: p.name })}
        />
        {runs !== undefined && (
          <RunsPanel
            platform={p.id}
            runs={runs}
            ended={job?.finished_ms ?? null}
            now={snapshot.now_ms}
            nameOf={nameOf}
            linkOf={(game) => gameLink(p.id, game)}
          />
        )}
      </>
    ),
    account: () => <AccountPanel name={p.name} state={account} actions={accountActions} />,
    setup: () => <SetupPanel link={(os) => setup(p.id, os)} />,
    activity: () => (
      <>
        {deleting !== undefined && <DeletingPanel job={deleting} name={nameOf(deleting.game)} now={snapshot.now_ms} />}
        {downloads.length > 0 ? (
          <DownloadList downloads={downloads} />
        ) : (
          deleting === undefined && <Quiet title={t("activity.idle", { name: p.name })} />
        )}
        {p.about.length > 0 && (
          <Panel title={t("activity.about", { name: p.name })}>
            <Notes items={p.about.map((fact) => t(`about.${fact}`))} />
          </Panel>
        )}
      </>
    ),
  }
  // The account part is for whoever signs lanstash in to platforms.
  const visible = partsOf(p).filter((s) => s !== "account" || can("platforms"))
  const shown = visible.includes(section) ? section : "cached"
  return (
    <View
      title={p.name}
      subtitle={t(`tagline.${p.described}`, { name: p.name })}
      icon={p.brand === undefined ? <Monogram letters={p.letters} /> : <Brand name={p.brand} size="large" />}
    >
      <Chips>
        <Chip value={size(p.bytes)} label={t("chip.stored")} tone="cache" />
        <Chip value={total > 0 ? percent(p.bytes / total) : "–"} label={t("chip.share")} />
        <Chip value={count(p.games.length)} label={t("chip.games", { count: p.games.length })} />
        <Chip value={count(p.objects)} label={t("chip.objects")} />
        {/* Quiet zeros: nothing downloading says nothing; nothing behind says so in words. */}
        {inFlight > 0 && <Chip value={count(inFlight)} label={t("chip.downloading")} tone="origin" />}
        {p.builds && builds !== undefined && behind > 0 && (
          <Chip value={count(behind)} label={behindLabel(t, p.buildsFor, builds.region)} tone="origin" />
        )}
        {p.builds && builds !== undefined && behind === 0 && p.games.length > 0 && (
          <Chip value={t("chip.all")} label={t("chip.current")} />
        )}
      </Chips>
      <Sections
        label={t("partsLabel", { name: p.name })}
        links={visible.map((s) => (
          <SectionLink
            key={s}
            href={sectionLink(p.id, s)}
            current={s === shown}
            label={t(`part.${s}`)}
            figure={figures[s]}
            dot={s === "prefill" || s === "account" || s === "activity" ? dots[s] : undefined}
          />
        ))}
      >
        {refused !== undefined && <Signal tone="bad">{refused}</Signal>}
        {parts[shown]()}
      </Sections>
    </View>
  )
}
