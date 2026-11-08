/**
 * Prefill panel: a platform's last prefill job and the games queued after it, drawn like a
 * launcher's downloads.
 *   - The job as a band with the game's art behind it: what it does (Prefill, Import, Import from
 *     lancache, Pre-load) over its name; the live readings: from the internet now, from lancache
 *     now (for an import), the peak so far and how much it has taken of what the stash lacked (the
 *     install less what was stashed already: a game held at 22 % reads as 78 % to take); a graph of
 *     the last moments, each bar split into lancache (amber) and the internet (cyan); the state
 *     (time left at the pace so far, how far the planning has come, or how it ended and what it
 *     took: a lancache import that found nothing says so) and the
 *     percentage over a bar; Stop while it runs.
 *   - Under it the other figures: from lancache and from the CDN in all, already stashed (its bytes), what was
 *     not there (on the CDN, or in lancache for a lancache-only import), pieces that failed their
 *     check, how long it has run; the file in flight on one line; why it failed.
 *   - Up next: the queued games in order, each with its art, its name (leading to its page), what it
 *     downloads where that is known, and Remove; Stop forgets them all.
 * Without a job it says how one starts. JobLine is the slim band for the top of another part: the
 * running job's art, name, state and percentage, and a link to the panel.
 */
import type { Prefill } from "@/contracts/prefill"
import { installName } from "@/features/platforms/install"
import { useLanguage, useSpan, useT, type Words } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { Button } from "@/styled/button"
import { Download, type Moment } from "@/styled/download"
import { Figure, Figures } from "@/styled/figures"
import { Queue, QueueItem } from "@/styled/queue"
import { Signal } from "@/styled/signal"
import { Link, Path, Quiet } from "@/styled/text"

/** What a job does, in a word or three. */
export function verb(t: Words<"platforms">, job: Prefill): string {
  if (job.prerelease) return t("prefill.verb.preload")
  if (job.from === "lancache-only") return t("prefill.verb.importLancache")
  if (job.from === "lancache") return t("prefill.verb.import")
  return t("prefill.verb.prefill")
}

/** The share of a job's requests done (0 while it is planned). */
const progress = (job: Prefill) => (job.files_total > 0 ? job.files_done / job.files_total : 0)

/** The time left at the pace so far, in seconds; undefined before there is a pace, or once it ended. */
export function timeLeft(job: Prefill, now: number): number | undefined {
  const took = (now - job.started_ms) / 1000
  if (job.state !== "running" || job.files_done === 0 || job.files_total === 0 || took <= 0) return undefined
  return ((job.files_total - job.files_done) * took) / job.files_done
}

/** The job as one snapshot saw it. */
export interface Sample {
  readonly at: number
  readonly job: Prefill | null | undefined
}

/** How fast a job goes: per moment, bytes a second from lancache and from the internet; now (over the last 5 s), and the peak. */
export interface Pace {
  readonly moments: readonly Moment[]
  readonly local: number
  readonly origin: number
  readonly peak: number
}

/** The pace of `job` from the samples (oldest first): only those of the same job count. */
export function pace(samples: readonly Sample[], job: Prefill): Pace {
  const mine = samples.filter((s) => s.job?.game === job.game && s.job.started_ms === job.started_ms)
  const moments: Moment[] = []
  for (const [i, b] of mine.entries()) {
    const a = mine[i - 1]
    if (a === undefined) continue
    const dt = (b.at - a.at) / 1000
    if (dt <= 0 || !a.job || !b.job) continue
    const local = Math.max(0, (b.job.bytes_local ?? 0) - (a.job.bytes_local ?? 0)) / dt
    const all = Math.max(0, b.job.bytes - a.job.bytes) / dt
    moments.push({ cache: local, origin: Math.max(0, all - local) })
  }
  // Now: over the last few seconds, as a job's bytes may grow a whole file at a time (Battle.net's are up to 16 MB).
  const newest = mine.at(-1)
  const oldest = mine.find((s) => newest !== undefined && newest.at - s.at <= NOW_WINDOW_MS)
  const dt = newest && oldest ? (newest.at - oldest.at) / 1000 : 0
  const grew = (f: (j: Prefill) => number) =>
    newest?.job && oldest?.job ? Math.max(0, f(newest.job) - f(oldest.job)) : 0
  const local = dt > 0 ? grew((j) => j.bytes_local ?? 0) / dt : 0
  const all = dt > 0 ? grew((j) => j.bytes) / dt : 0
  return {
    moments,
    local,
    origin: Math.max(0, all - local),
    peak: Math.max(0, ...moments.map((m) => (m.cache ?? 0) + (m.origin ?? 0))),
  }
}

/** How far back the pace now looks. */
const NOW_WINDOW_MS = 5000

/** Bytes a second, in bits as a network meter shows them. */
export const bitrate = (bytesPerSecond: number) => {
  const bits = bytesPerSecond * 8
  if (bits >= 1e9) return `${(bits / 1e9).toFixed(2)} Gbps`
  if (bits >= 1e6) return `${(bits / 1e6).toFixed(1)} Mbps`
  if (bits >= 1e3) return `${(bits / 1e3).toFixed(0)} kbps`
  return `${bits.toFixed(0)} bps`
}

/** A job's game, and the platform it is for when not the usual one ("Diablo IV · for macOS"). */
export const titled = (t: Words<"platforms">, name: string, job: Prefill, language: string) =>
  job.install == null ? name : `${name} · ${t("install.for", { install: installName(job.install, language) })}`

/** The line over a job's bar: how it stands. */
function stateOf(t: Words<"platforms">, span: (seconds: number) => string, job: Prefill, now: number): string {
  const took = ((job.finished_ms ?? now) - job.started_ms) / 1000
  if (job.state === "running") {
    if (job.files_total === 0) {
      const plan = job.planning
      return plan && plan.total > 0
        ? t("prefill.state.planningIndexes", { done: count(plan.done), total: count(plan.total) })
        : t("prefill.state.planning")
    }
    const left = timeLeft(job, now)
    const requests = t("prefill.state.requests", { done: count(job.files_done), total: count(job.files_total) })
    return left === undefined ? requests : t("prefill.state.left", { left: span(left), requests })
  }
  if (job.state === "done") {
    // What it took, so a job that found nothing does not read as a finished download.
    const local = job.bytes_local ?? 0
    const fetched = Math.max(0, job.bytes - local)
    const what =
      job.from === "lancache-only"
        ? local === 0
          ? t("prefill.took.lancacheNone")
          : t("prefill.took.lancacheOnly", { size: size(local) })
        : local === 0 && fetched === 0
          ? job.files_skipped === job.files_done
            ? t("prefill.took.allStashed")
            : t("prefill.took.nothing")
          : [
              local > 0 && t("prefill.took.lancache", { size: size(local) }),
              fetched > 0 && t("prefill.took.cdn", { size: size(fetched) }),
              (job.bytes_skipped ?? 0) > 0 && t("prefill.took.stashed", { size: size(job.bytes_skipped ?? 0) }),
            ]
              .filter(Boolean)
              .join(t("prefill.took.and"))
    return t("prefill.state.done", { took: span(took), what })
  }
  if (job.state === "stopped") return t("prefill.state.stopped", { took: span(took) })
  return t("prefill.state.failed", { took: span(took) })
}

export interface PrefillPanelProps {
  /** The platform's last job; none yet: `idle` says how one starts. */
  readonly job: Prefill | null | undefined
  /** The job's game, by name. */
  readonly name: string
  /** A queued game's name. */
  readonly nameOf?: (game: string) => string
  /** A game's art, where there is any. */
  readonly artOf?: (game: string) => string | undefined
  /** What prefilling a game downloads, where that is known. */
  readonly sizeOf?: (game: string) => number | undefined
  /** A game's page. */
  readonly linkOf?: (game: string) => string
  /** The job as the last snapshots saw it, oldest first: its pace. */
  readonly samples?: readonly Sample[]
  /** The time now (ms). */
  readonly now: number
  /** Stops the job; no Stop without it (someone who may not change things). */
  readonly onStop?: () => void
  /** Takes a queued game out of the queue; no Remove without it. */
  readonly onUnqueue?: (game: string) => void
  /** What to say without a job. */
  readonly idle?: string
  /** Whether the queue is listed under the job (not on a game's own page). */
  readonly queue?: boolean
}

export function PrefillPanel({
  job,
  name,
  nameOf = (g) => g,
  artOf = () => undefined,
  sizeOf = () => undefined,
  linkOf,
  samples = [],
  now,
  onStop,
  onUnqueue,
  idle,
  queue: listQueue = true,
}: PrefillPanelProps) {
  const t = useT("platforms")
  const { code: language } = useLanguage()
  const span = useSpan()
  if (job === null || job === undefined) {
    return <Quiet title={t("prefill.none")} hint={idle} />
  }
  const running = job.state === "running"
  // Done: the state line over the bar tells the sizes and the time.
  const told = job.state === "done"
  const took = ((job.finished_ms ?? now) - job.started_ms) / 1000
  const imports = job.from === "lancache" || job.from === "lancache-only"
  const local = job.bytes_local ?? 0
  const queued = job.queued ?? []
  const p = pace(samples, job)
  const install = job.bytes_total ?? 0
  // What the stash held already is not downloaded: the total is what is left to take.
  const stashed = job.bytes_skipped ?? 0
  const wanted = Math.max(job.bytes, install - stashed)
  return (
    <>
      <Download
        title={titled(t, name, job, language)}
        doing={verb(t, job)}
        image={artOf(job.game)}
        readings={[
          // The rates only while it runs; after, the peak where one was seen.
          ...(running
            ? [
                { label: t("prefill.reading.network"), value: bitrate(p.origin), tone: "origin" as const },
                ...(imports ? [{ label: "Lancache", value: bitrate(p.local), tone: "cache" as const }] : []),
              ]
            : []),
          ...(running || p.peak > 0 ? [{ label: t("prefill.reading.peak"), value: bitrate(p.peak) }] : []),
          {
            label: t("prefill.reading.taken"),
            value: install > 0 ? t("game.of", { have: size(job.bytes), want: size(wanted) }) : size(job.bytes),
          },
        ]}
        moments={p.moments}
        fraction={progress(job)}
        state={stateOf(t, span, job, now)}
        percent={`${(100 * progress(job)).toFixed(1)} %`}
        action={
          running && onStop !== undefined ? (
            <Button
              onClick={onStop}
              label={t(queued.length > 0 ? "prefill.stopAllLabel" : "prefill.stopLabel", { name })}
            >
              {t("button.stop")}
            </Button>
          ) : undefined
        }
      />
      {/* A finished job's line over the bar says what came from where and how long it took. */}
      {(!told || job.files_missing > 0 || (job.files_rejected ?? 0) > 0) && (
        <Figures>
          {!told && imports && <Figure label={t("prefill.figure.lancache")} value={size(local)} tone="cache" />}
          {!told && (
            <Figure label={t("prefill.figure.cdn")} value={size(Math.max(0, job.bytes - local))} tone="origin" />
          )}
          {!told && (
            <Figure
              label={t("prefill.figure.stashed")}
              value={job.bytes_skipped === undefined ? count(job.files_skipped) : size(job.bytes_skipped)}
            />
          )}
          {job.files_missing > 0 && (
            <Figure
              label={t(job.from === "lancache-only" ? "prefill.figure.notInLancache" : "prefill.figure.notOnCdn")}
              value={count(job.files_missing)}
            />
          )}
          {(job.files_rejected ?? 0) > 0 && (
            <Figure label={t("prefill.figure.rejected")} value={count(job.files_rejected ?? 0)} />
          )}
          {!told && <Figure label={t(running ? "prefill.figure.running" : "prefill.figure.took")} value={span(took)} />}
          {!told && job.version !== null && (
            <Figure
              label={t(job.prerelease ? "prefill.figure.prerelease" : "prefill.figure.version")}
              value={job.version}
            />
          )}
        </Figures>
      )}
      {running && job.current !== null && <Path>{job.current}</Path>}
      {job.error !== null && <Signal tone="bad">{job.error}</Signal>}
      {listQueue && (
        <Queue
          title={t("prefill.queue.title")}
          count={queued.length}
          note={queued.length > 0 ? t("prefill.queue.note") : undefined}
          empty={t("prefill.queue.empty")}
        >
          {queued.map((g) => {
            const bytes = sizeOf(g)
            return (
              <QueueItem
                key={g}
                name={nameOf(g)}
                href={linkOf?.(g)}
                image={artOf(g)}
                detail={bytes === undefined ? g : `${size(bytes)} · ${g}`}
                actions={
                  onUnqueue === undefined ? undefined : (
                    <Button onClick={() => onUnqueue(g)} label={t("prefill.queue.removeLabel", { name: nameOf(g) })}>
                      {t("prefill.queue.remove")}
                    </Button>
                  )
                }
              />
            )
          })}
        </Queue>
      )}
    </>
  )
}

export interface JobLineProps {
  /** The running job. */
  readonly job: Prefill
  /** Its game, by name. */
  readonly name: string
  /** Its art, if any. */
  readonly image?: string
  /** The time now (ms). */
  readonly now: number
  /** Where the whole panel is. */
  readonly href: string
}

/** The running job as a slim band: what, how far, and a link to the panel. */
export function JobLine({ job, name, image, now, href }: JobLineProps) {
  const t = useT("platforms")
  const { code: language } = useLanguage()
  const span = useSpan()
  const queued = job.queued?.length ?? 0
  return (
    <Download
      compact
      title={titled(t, name, job, language)}
      doing={queued > 0 ? t("prefill.more", { verb: verb(t, job), n: count(queued) }) : verb(t, job)}
      image={image}
      fraction={progress(job)}
      state={stateOf(t, span, job, now)}
      percent={`${(100 * progress(job)).toFixed(1)} %`}
      action={<Link href={href}>{t("prefill.details")}</Link>}
    />
  )
}
