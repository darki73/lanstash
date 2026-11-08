/**
 * Pulse: the line over the overview that says what lanstash is doing, at a glance: serving clients
 * (how many, how fast), downloading, prefilling, or idle (and when clients last got anything, from
 * what was kept); how its own checks stand, linking to them (a failing one turns the line red);
 * and on the right the switch of the range the whole overview shows. The checks are read once and
 * again every minute.
 */
import { useEffect, useState } from "react"
import { type HealthCheck, type HealthReader, worst } from "@/contracts/health"
import type { Snapshot } from "@/contracts/status"
import { nowRows } from "@/features/clients"
import { dirGame } from "@/features/platforms/model"
import type { Kept } from "@/features/status/kept"
import { many, objectsInFlight } from "@/features/status/model"
import { type Range, rangeChoices, spanOf } from "@/features/status/range"
import { useAgo, useT, type Words } from "@/i18n"
import { Segmented } from "@/styled/segmented"
import { StatusLine, type StatusMood } from "@/styled/statusline"
import { Link } from "@/styled/text"

/** The words for what lanstash is doing now, and whether it is busy. */
export function doing(s: Snapshot, t: Words<"status">): { busy: boolean; words: string } {
  // Only counted here: what a game is called does not matter.
  const clients = nowRows(s.clients ?? [], (p, d) => dirGame(s.usage, p, d, d))
  if (clients.length > 0) {
    const people = new Set(clients.map((c) => c.client.address)).size
    const perSec = clients.reduce((n, c) => n + c.perSec, 0)
    return { busy: true, words: t("pulse.serving", { ...many(people), rate: (perSec / 1e6).toFixed(1) }) }
  }
  const objects = objectsInFlight(s)
  if (objects > 0) return { busy: true, words: t("pulse.downloading", many(objects)) }
  const prefilling = Object.values(s.prefills ?? {}).filter((p) => p.state === "running").length
  if (prefilling > 0 || s.prefill?.state === "running") {
    return { busy: true, words: t("pulse.prefilling", many(Math.max(1, prefilling))) }
  }
  return { busy: false, words: t("pulse.idle") }
}

/** How the checks stand, in words. */
export function checksWords(checks: readonly HealthCheck[], t: Words<"status">): string {
  const failing = checks.filter((c) => c.level === "fail").length
  const warning = checks.filter((c) => c.level === "warn").length
  if (failing > 0) return t("pulse.failing", many(failing))
  if (warning > 0) return t("pulse.warnings", many(warning))
  return t("pulse.pass")
}

export interface PulseProps {
  readonly snapshot: Snapshot
  readonly kept: Kept | null | undefined
  readonly health?: HealthReader
  readonly range: Range
  readonly onRange: (range: Range) => void
  /** Where the checks are. */
  readonly healthHref: string
  /** How often the checks are read again (ms); a test passes 0 for once. */
  readonly every?: number
}

export function Pulse({ snapshot, kept, health, range, onRange, healthHref, every = 60_000 }: PulseProps) {
  const t = useT("status")
  const ago = useAgo()
  const [checks, setChecks] = useState<readonly HealthCheck[]>()
  useEffect(() => {
    if (health === undefined) return
    let live = true
    const read = () =>
      void health().then((c) => {
        if (live) setChecks(c)
      })
    read()
    const timer = every > 0 ? setInterval(read, every) : undefined
    return () => {
      live = false
      if (timer !== undefined) clearInterval(timer)
    }
  }, [health, every])
  const now = doing(snapshot, t)
  const last = kept ? kept.lastRise(["requests.hit_bytes", "requests.miss_bytes"]) : undefined
  const facts = [
    ...(now.busy || last === undefined || kept == null
      ? []
      : [
          last === 0
            ? t("pulse.noTraffic", { range: spanOf(t, kept.range) })
            : t("pulse.lastGot", { ago: ago(snapshot.now_ms, last) }),
        ]),
    ...(checks === undefined
      ? []
      : [
          <Link key="checks" href={healthHref}>
            {checksWords(checks, t)}
          </Link>,
        ]),
  ]
  const mood: StatusMood = checks !== undefined && worst(checks) === "fail" ? "trouble" : now.busy ? "busy" : "idle"
  return (
    <StatusLine
      mood={mood}
      words={now.words}
      facts={facts}
      action={<Segmented label={t("range.label")} options={rangeChoices(t)} value={range} onChange={onRange} />}
    />
  )
}
