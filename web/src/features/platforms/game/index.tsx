/**
 * Game page: one game of a platform, cached or not, laid out the same on every platform.
 *   - A showcase band: the game's art, where it is from (platform and product), its name, its facts
 *     (stored, objects, last used, the CDN region where lanstash checks builds), its current build's
 *     badge, and the main build's buttons: Prefill (Queue while the platform prefills another game,
 *     Queued once waiting, Stop on the game being prefilled) and, where lanstash can read lancache's
 *     old cache, probed it for the build and it holds more of it than the stash, From lancache.
 *   - The prefill job, when it is this game's (or one of its directory's products'), as a band.
 *   - Builds (where lanstash can prefill): a row per build (the game's own, a pre-release one with
 *     Pre-load, each other product of its directory by the name the launcher gives it, with its own
 *     buttons): its version, badge, how much of it is stashed, why it could not be worked out, what
 *     lancache's old cache holds of it (Steam), and its details folded away: whole files and files
 *     inside archives, what a prefill sends, what the CDN lacks, its build config.
 *   - What is downloading for it right now, when anything is.
 *   - On the side: what the stash holds of it with Delete (asked once more), its old builds to prune,
 *     and a deletion of it while one runs.
 * A game neither cached nor known says so.
 */
import { type ReactNode, useState } from "react"
import { type ArtLink, noArt } from "@/contracts/art"
import type { BuildGame, Builds } from "@/contracts/builds"
import type { Catalog, CatalogProduct } from "@/contracts/catalog"
import type { ClientsReader } from "@/contracts/clients"
import type { GameBuild, GameDetail } from "@/contracts/game"
import { eventsOf, type History, noHistory } from "@/contracts/history"
import { noPrefill, type PrefillActions, running, waiting } from "@/contracts/prefill"
import type { PrivateActions } from "@/contracts/private"
import { recentDeletion, type StashActions } from "@/contracts/stash"
import type { Snapshot } from "@/contracts/status"
import { useAccess } from "@/features/access"
import { WhoGotIt } from "@/features/clients"
import { days } from "@/features/history"
import { DeletingPanel } from "@/features/platforms/deleting"
import { Install, named } from "@/features/platforms/install"
import { jobOf, owns, platform, platformLink, worthLancache } from "@/features/platforms/model"
import { PrefillPanel } from "@/features/platforms/prefill"
import { PrunePanel } from "@/features/platforms/prune"
import { HidePanel, usePrivacy } from "@/features/privacy"
import { PrivatePanel } from "@/features/privacy/private"
import { DownloadList } from "@/features/status/downloads/list"
import { useAgo, useT, type Words } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { done } from "@/lib/percent"
import { BuildList, BuildRow } from "@/styled/builds"
import { Button } from "@/styled/button"
import { Panel } from "@/styled/panel"
import { Readings } from "@/styled/readings"
import { Showcase } from "@/styled/showcase"
import { Signal } from "@/styled/signal"
import { Split } from "@/styled/split"
import { Muted, Quiet } from "@/styled/text"
import { Timeline } from "@/styled/timeline"

export interface GameViewProps {
  readonly snapshot: Snapshot
  readonly platformId: string
  /** The game's id on the platform (a Battle.net CDN directory, a Steam app id). */
  readonly gameId: string
  /** What the API says about the game (Battle.net), once it said it. */
  readonly detail?: GameDetail
  readonly builds?: Builds
  readonly catalog?: Catalog
  readonly art?: ArtLink
  readonly prefill?: PrefillActions
  /** Deletes the game from the stash; no Delete without it. */
  readonly stash?: StashActions
  /** The latest snapshots, oldest first: a running prefill's pace (none: no pace shown). */
  readonly history?: readonly Snapshot[]
  /** What was updated when, of every game (the game's own are shown); none: no updates shown. */
  readonly updates?: History
  /** Whether lanstash keeps the game up to date on its own; undefined when that is off or not known. */
  readonly followed?: boolean
  /** What each client got over a range (who got the game is shown); none: not shown. */
  readonly clients?: ClientsReader
  /** The signed-in player's private games; none: not offered. */
  readonly privateGames?: PrivateActions
}

/**
 * What the stash holds of the game, and Delete: asked once more ("Delete 5.78 GB?") before it
 * goes, refused while the game is being prefilled; what went wrong, if anything.
 */
function StashPanel(props: { name: string; bytes: number; busy: boolean; remove: () => Promise<string | undefined> }) {
  const t = useT("platforms")
  const [step, setStep] = useState<"idle" | "asking" | "deleting">("idle")
  const [failed, setFailed] = useState<string>()
  const go = () => {
    setStep("deleting")
    void props.remove().then((why) => {
      setFailed(why)
      setStep("idle")
    })
  }
  const action =
    step === "asking" ? (
      <>
        <Button onClick={() => setStep("idle")}>{t("game.stash.keep")}</Button>
        <Button onClick={go} tone="danger" label={t("game.stash.deleteLabel", { name: props.name })}>
          {t("game.stash.deleteSize", { size: size(props.bytes) })}
        </Button>
      </>
    ) : (
      <Button
        onClick={() => setStep("asking")}
        disabled={props.busy || step === "deleting"}
        label={t("game.stash.askLabel", { name: props.name })}
      >
        {step === "deleting" ? t("game.stash.deleting") : t("game.stash.ask")}
      </Button>
    )
  return (
    <Panel title={t("game.stash.title")} note={size(props.bytes)} action={action}>
      <Muted>{step === "asking" ? t("game.stash.asking") : t("game.stash.idle")}</Muted>
      {failed !== undefined && <Signal tone="bad">{failed}</Signal>}
    </Panel>
  )
}

/** A build's state as a badge. */
function badge(t: Words<"platforms">, b: GameBuild) {
  if (b.complete) {
    return (
      <Signal tone="good" still>
        {t("build.current")}
      </Signal>
    )
  }
  if (b.have === null || b.want === null) {
    return b.plan_error === null ? (
      <Signal tone="waiting">{t("build.checking")}</Signal>
    ) : (
      <Signal tone="bad">{t("build.unknown")}</Signal>
    )
  }
  return (
    <Signal tone="waiting" still>
      {done(b.have, b.want)}
    </Signal>
  )
}

/** A build's details: how much of its install the stash holds, and in what. */
function details(t: Words<"platforms">, b: GameBuild) {
  const d = b.detail
  if (d === null || b.have === null || b.want === null) return undefined
  const of = (have: number, want: number) => t("game.of", { have: size(have), want: size(want) })
  return (
    <Readings
      rows={[
        {
          name: t("game.detail.wholeFiles", { n: count(d.file_count) }),
          value: of(d.files_have, d.files_want),
        },
        {
          name: t("game.detail.archives", { archives: count(d.archive_count), files: count(d.part_count) }),
          value: of(d.archived_have, d.archived_want),
        },
        { name: t("game.detail.requests"), value: count(d.requests) },
        { name: t("game.detail.absent"), value: count(d.absent) },
        { name: t("game.detail.config"), value: b.build_config.slice(0, 12) },
      ]}
    />
  )
}

/** A build as a row: name, version, badge, how much is stashed, why it is not known, details. */
function row(
  t: Words<"platforms">,
  key: string,
  name: string,
  tag: string | undefined,
  b: GameBuild,
  image: string | undefined,
  actions?: ReactNode,
  note?: string,
) {
  const known = b.have !== null && b.want !== null
  return (
    <BuildRow
      key={key}
      name={name}
      image={image}
      tag={tag}
      version={t("game.version", { version: b.version })}
      badge={badge(t, b)}
      fraction={known && b.want !== null && b.want > 0 ? (b.have ?? 0) / b.want : undefined}
      amount={known ? t("game.of", { have: size(b.have ?? 0), want: size(b.want ?? 0) }) : undefined}
      note={
        known ? (
          note === undefined ? undefined : (
            <Muted>{note}</Muted>
          )
        ) : (
          <Signal tone={b.plan_error === null ? "waiting" : "bad"}>{b.plan_error ?? t("game.planning")}</Signal>
        )
      }
      actions={actions}
      details={details(t, b)}
    />
  )
}

/**
 * A build as the game page draws it, where the platform has no game detail (Steam): from the builds
 * view for a game in the stash, else from the catalog for one that is not (nothing of it stashed).
 */
export function fallbackBuild(cached: BuildGame | undefined, listed: CatalogProduct | undefined): GameBuild | null {
  if (cached?.version != null) {
    return {
      version: cached.version,
      build_config: "",
      have: cached.have ?? null,
      want: cached.want ?? null,
      complete: cached.current,
      plan_error: cached.plan_error ?? null,
      detail: null,
    }
  }
  if (listed?.version != null) {
    return {
      version: listed.version,
      build_config: "",
      have: listed.download === undefined ? null : 0,
      want: listed.download ?? null,
      complete: false,
      plan_error: null,
      detail: null,
    }
  }
  return null
}

/** About how much of a game lancache's old cache holds, where it was probed. */
function lancacheLine(
  t: Words<"platforms">,
  cached: BuildGame | undefined,
  listed: CatalogProduct | undefined,
): string | undefined {
  const s = cached?.lancache_share ?? listed?.lancache
  if (s == null || s.want === 0 || cached?.current === true) return undefined
  return s.have === 0 ? t("game.lancacheNone") : t("game.lancacheSome", { share: done(s.have, s.want) })
}

/** How long a start just asked for waits for its job to show before its button is offered again. */
const startingMs = 5_000

export function GameView(props: GameViewProps) {
  const { snapshot, platformId, gameId, detail, builds, catalog, art = noArt, prefill = noPrefill, stash } = props
  const history = props.history ?? []
  const updates = props.updates ?? noHistory
  const ago = useAgo()
  const t = useT("platforms")
  const tCommon = useT("common")
  const tHistory = useT("history")
  const [refused, setRefused] = useState<string>()
  // A start just asked for (product, the snapshot's time then): until the job shows, its button
  // waits disabled, so the second click of a double click never lands on the Stop that follows.
  const [sent, setSent] = useState<{ readonly id: string; readonly at: number }>()
  // The product whose Stop was clicked once: stopping is asked once more.
  const [stopping, setStopping] = useState<string>()
  const privacy = usePrivacy()
  const { can, session } = useAccess()
  const p = platform(snapshot.usage, platformId, builds)
  if (p === undefined) {
    return <Quiet title={t("noPlatform.title", { id: platformId })} hint={t("noPlatform.hint")} />
  }
  const cached = p.games.find((g) => owns(g, gameId))
  if (privacy.of(p.id, gameId) === "masked" || cached?.privacy === "masked") {
    return (
      <Quiet
        title={t("game.masked.title", { game: tCommon("hiddenGame"), platform: p.name })}
        hint={t("game.masked.hint")}
      />
    )
  }
  const listed = catalog?.products.find((c) => c.game === gameId)
  const name = cached?.named ? cached.name : (listed?.name ?? cached?.name ?? gameId)
  const recent = recentDeletion(snapshot.deleting, snapshot.now_ms)
  const deleting = recent?.platform === p.id && (cached?.codes ?? [gameId]).includes(recent.game) ? recent : undefined
  const deletingPanel = deleting && (
    <DeletingPanel
      job={deleting}
      name={cached?.name ?? listed?.name ?? gameId}
      now={snapshot.now_ms}
      back={{ href: platformLink(p.id), label: t("game.back", { name: p.name }) }}
    />
  )
  if (cached === undefined && listed === undefined && detail?.product == null) {
    if (deletingPanel) return deletingPanel
    return (
      <Quiet
        title={t("game.none.title", { id: gameId, platform: p.name })}
        hint={t(p.prefill ? "game.none.hintWithCatalog" : "game.none.hint")}
      />
    )
  }
  const artId = cached?.art ?? listed?.code ?? (p.art?.by === "code" ? gameId : undefined)
  const image = artId === undefined || p.art === undefined ? undefined : art(p.id, artId)
  const icon = artId === undefined || !p.art?.icon ? undefined : art(p.id, artId, "icon")
  const background = artId === undefined || p.art === undefined ? undefined : art(p.id, artId, "background")
  const logo = artId === undefined || p.art === undefined ? undefined : art(p.id, artId, "logo")
  // Another product of the directory, by its own background.
  const artOf = (code: string) => (p.art === undefined ? undefined : art(p.id, code, "background"))
  const job = jobOf(snapshot, p.id)
  const flavours = detail?.flavours ?? []
  // Other installs to prefill: Battle.net's build's platforms, else the operating systems a Steam app installs for.
  const installs =
    detail?.installs ?? (listed?.systems === undefined ? null : { platforms: listed.systems, architectures: [] })
  const current = detail?.build ?? (detail === undefined ? fallbackBuild(cached?.build, listed) : null)
  const lancacheOk = detail?.lancache ?? builds?.lancache ?? false
  // What lancache's old cache holds of the game's own current build, once probed (a pre-release build is not probed).
  const share = cached?.build?.lancache_share ?? listed?.lancache
  const inLancache = detail === undefined ? lancacheLine(t, cached?.build, listed) : undefined
  // The game's own product, and the other products of its directory (prefilled by their codes).
  const ids = [gameId, ...flavours.map((f) => f.product)]
  const mine = running(job) && ids.includes(job.game)
  const start = (pre: boolean, fromLancache = false, id = gameId) => {
    setRefused(undefined)
    setSent({ id, at: snapshot.now_ms })
    // It queues: it runs at once when the platform is idle, else after the running job.
    void prefill.start(p.id, id, pre, fromLancache, true).then((why) => {
      setRefused(why)
      if (why !== undefined) setSent(undefined)
    })
  }
  const downloads = snapshot.downloads.filter((d) =>
    (cached?.codes ?? [gameId]).some((c) => d.key.split("/").includes(c)),
  )
  /** The buttons of build `b` of product `id` (the game's own, or a flavour's), called `label`. */
  // Someone who may not change things gets no Prefill nor Stop.
  // A player's catalog holds only its own games: those it may prefill.
  const mayPrefill = can("prefill") || (can("prefill.own") && listed !== undefined)
  const action = (pre: boolean, b: GameBuild, id = gameId, label = name) => {
    if (!mayPrefill) return undefined
    const its = running(job) && job.game === id
    // Stopping a prefill needs `prefill`: one who may only start its own games gets no Stop.
    if (its && !can("prefill")) return undefined
    const busy = running(job) && !its
    if (!its && waiting(job, id)) {
      return (
        <Button onClick={() => {}} disabled label={t("button.queuedLabel", { name: label })}>
          {t("button.queued")}
        </Button>
      )
    }
    // Asked for, not shown yet (a few seconds at most: a job that ended at once shows no more).
    if (!its && sent?.id === id && snapshot.now_ms - sent.at < startingMs) {
      return (
        <Button onClick={() => {}} disabled label={t("button.startingLabel", { name: label })}>
          {t("button.starting")}
        </Button>
      )
    }
    if (its && stopping === id) {
      return (
        <>
          <Button onClick={() => setStopping(undefined)}>{t("button.keepGoing")}</Button>
          <Button
            onClick={() => {
              setStopping(undefined)
              void prefill.stop(p.id)
            }}
            tone="danger"
            label={t("button.stopNowLabel", { name: label })}
          >
            {t("button.stopNow")}
          </Button>
        </>
      )
    }
    return its ? (
      <Button onClick={() => setStopping(id)} label={t("button.stopAskLabel", { name: label })}>
        {t("button.stopAsk")}
      </Button>
    ) : (
      <>
        {!b.complete && worthLancache(lancacheOk, !pre && id === gameId ? share : undefined, b.have) && (
          <Button
            onClick={() => start(pre, true, id)}
            label={t(
              busy
                ? "button.queueFromLancacheLabel"
                : pre
                  ? "button.preloadFromLancacheLabel"
                  : "button.prefillFromLancacheLabel",
              { name: label },
            )}
          >
            {t("button.fromLancache")}
          </Button>
        )}
        <Button
          onClick={() => start(pre, false, id)}
          tone={b.complete ? "plain" : "accent"}
          label={
            busy
              ? t("button.queueLabel", { name: label })
              : pre
                ? t("button.preloadLabel", { name: label, version: b.version })
                : t("button.prefillLabel", { name: label })
          }
        >
          {t(busy ? "button.queue" : pre ? "button.preload" : "button.prefill")}
        </Button>
      </>
    )
  }
  const product = detail?.product ?? listed?.code
  const facts = [
    { label: t("game.fact.stored"), value: cached ? size(cached.bytes) : "–" },
    { label: t("game.fact.objects"), value: count(cached?.objects ?? 0) },
    { label: t("game.fact.used"), value: cached ? ago(snapshot.now_ms, cached.lastUsedMs) : tCommon("ago.never") },
    ...(detail ? [{ label: t("game.fact.region"), value: detail.region.toUpperCase() }] : []),
    // Followed once held whole: its new builds come on their own.
    ...(props.followed === undefined
      ? []
      : [{ label: t("game.fact.updates"), value: t(props.followed ? "game.fact.automatic" : "game.fact.byHand") }]),
  ]
  // Other platforms' installs of the current build the stash holds some of, each a row of its own.
  const others = Object.entries(installs?.held ?? {}).flatMap(([platform, h]) =>
    platform !== installs?.own && h != null && h.have > 0 ? [{ platform, ...h }] : [],
  )
  // With other platforms' rows, the usual one says its platform too.
  const ownName = others.length > 0 && installs?.own != null ? `${name} · ${named(installs.own)}` : name
  const rows = [
    ...(current != null ? [row(t, "current", ownName, product, current, background, undefined, inLancache)] : []),
    ...(current != null
      ? others.map((o) =>
          row(
            t,
            `install-${o.platform}`,
            `${name} · ${named(o.platform)}`,
            product,
            {
              ...current,
              have: o.have,
              want: o.want,
              complete: o.have >= o.want,
              plan_error: null,
              detail: null,
            },
            background,
          ),
        )
      : []),
    ...(detail?.prerelease != null
      ? [row(t, "pre", t("game.prerelease"), product, detail.prerelease, background, action(true, detail.prerelease))]
      : []),
    ...flavours.flatMap((f) => {
      const label = f.name ?? f.product
      return [
        ...(f.build !== null
          ? [row(t, f.product, label, f.product, f.build, artOf(f.product), action(false, f.build, f.product, label))]
          : []),
        ...(f.prerelease !== null
          ? [
              row(
                t,
                `${f.product}-pre`,
                t("game.flavourPrerelease", { name: label }),
                f.product,
                f.prerelease,
                artOf(f.product),
                action(true, f.prerelease, f.product, label),
              ),
            ]
          : []),
      ]
    }),
  ]
  const ownJob = job != null && ids.includes(job.game) ? job : null
  // What was updated when, of the game and the other products of its directory.
  const own = new Set([...ids, ...(cached?.codes ?? []), ...(cached?.id === undefined ? [] : [cached.id])])
  const mineUpdates = { events: [...own].flatMap((g) => eventsOf(updates, p.id, g)).sort((a, b) => b.at_ms - a.at_ms) }
  const catalogs = catalog === undefined ? undefined : { [p.id]: catalog }
  const side = (
    <>
      {deletingPanel}
      {deletingPanel === undefined && stash !== undefined && cached !== undefined && cached.bytes > 0 && (
        <StashPanel
          name={name}
          bytes={cached.bytes}
          busy={mine}
          remove={async () => {
            // One deletion of the whole game: lanstash deletes all its parts (a Steam app's depots).
            return stash.remove(p.id, cached.id ?? gameId)
          }}
        />
      )}
      {p.prune && stash?.prunable && stash.prune && cached !== undefined && (
        <PrunePanel
          name={name}
          check={() => stash.prunable?.(p.id, gameId) ?? Promise.resolve(t("game.noPrune"))}
          prune={() => stash.prune?.(p.id, gameId) ?? Promise.resolve(t("game.noPrune"))}
          busy={mine || recent?.state === "running"}
        />
      )}
      <HidePanel platform={p.id} game={cached?.id ?? gameId} />
      {/* A player's own game (its catalog lists only those): kept to itself or not. Admins hide instead. */}
      {props.privateGames !== undefined && !can("hide") && can("prefill.own") && listed !== undefined && (
        <PrivatePanel platform={p.id} game={listed.game ?? gameId} actions={props.privateGames} />
      )}
    </>
  )
  return (
    <Showcase
      title={name}
      eyebrow={[p.name, product].filter(Boolean).join(" · ")}
      art={[
        ...(background === undefined ? [] : [{ src: background, cover: "wide" as const }]),
        ...(image === undefined ? [] : [{ src: image, cover: p.art?.cover ?? "wide" }]),
      ]}
      logo={logo}
      icon={icon}
      badge={
        // Where lanstash checks no builds (Epic) and knows no size, there is nothing being checked.
        current == null || (!p.builds && current.want === null) ? undefined : badge(t, current)
      }
      facts={facts}
      actions={p.prefill && current != null ? action(false, current) : undefined}
    >
      {refused !== undefined && <Signal tone="bad">{refused}</Signal>}
      {p.prefill && ownJob !== null && (
        <PrefillPanel
          job={ownJob}
          name={flavours.find((f) => f.product === ownJob.game)?.name ?? name}
          artOf={() => image}
          now={snapshot.now_ms}
          onStop={can("prefill") ? () => void prefill.stop(p.id) : undefined}
          queue={false}
          samples={history.map((s) => ({ at: s.now_ms, job: jobOf(s, p.id) }))}
        />
      )}
      <Split side={side}>
        {p.prefill && rows.length > 0 && (
          <Panel
            title={t("game.builds.title")}
            note={rows.length > 1 ? t("game.builds.note", { n: count(rows.length) }) : undefined}
          >
            <BuildList label={t("game.builds.label", { name })}>{rows}</BuildList>
          </Panel>
        )}
        {p.prefill && detail?.error != null && <Signal tone="bad">{detail.error}</Signal>}
        {p.prefill && installs != null && mayPrefill && (
          <Install
            installs={installs}
            name={name}
            start={(install) => {
              setRefused(undefined)
              setSent({ id: gameId, at: snapshot.now_ms })
              void prefill.start(p.id, gameId, false, false, true, install).then((why) => {
                setRefused(why)
                if (why !== undefined) setSent(undefined)
              })
            }}
          />
        )}
        {downloads.length > 0 && <DownloadList downloads={downloads} />}
        {mineUpdates.events.length > 0 && (
          <Panel title={t("game.updates.title")}>
            <Timeline
              label={t("game.updates.label", { name })}
              days={days({ history: mineUpdates, snapshot, catalogs }, tHistory, tCommon("hiddenGame"))}
            />
          </Panel>
        )}
        {/* Clients are for signed-in accounts: a reader is not asked. */}
        {props.clients !== undefined && (!session.accounts || session.role !== "guest") && (
          <WhoGotIt
            clients={props.clients}
            mine={(service, dir) => service === p.id && own.has(dir)}
            at={snapshot.now_ms}
          />
        )}
      </Split>
    </Showcase>
  )
}
