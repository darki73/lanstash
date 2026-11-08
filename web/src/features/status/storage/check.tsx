/**
 * Check: the background store check (every stored byte read against its checksum, paced so clients
 * keep the disks), when it is set on: while a round runs, how far it is; else when the last one
 * finished and when the next is due; and the damage the last round found, which lanstash's log
 * names file by file. With the check off it shows nothing.
 */
import type { Snapshot } from "@/contracts/status"
import { day } from "@/features/status/lifetime"
import { many } from "@/features/status/model"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { Figure, Figures } from "@/styled/figures"
import { Signal } from "@/styled/signal"

export function Check({ snapshot }: { readonly snapshot: Snapshot }) {
  const t = useT("status")
  const v = snapshot.counters.verify
  if (v === undefined) return null
  const running = (v.running ?? 0) > 0
  const [done, all] = [v.bytes_done ?? 0, v.bytes ?? 0]
  const finished = v.finished_ms ?? 0
  const damage = v.problems ?? 0
  const state = running
    ? t("storage.check.checking", { percent: Math.floor((all === 0 ? 0 : done / all) * 100) })
    : finished > 0
      ? t("storage.check.checked", { day: day(finished, t) })
      : t("storage.check.notYet")
  const next = finished > 0 ? day(finished + (v.every_ms ?? 0), t) : t("storage.check.withinMinutes")
  return (
    <>
      <Figures>
        <Figure label={t("storage.check.label")} value={state} />
        <Figure
          label={running ? t("storage.check.read") : t("storage.check.next")}
          value={running ? t("storage.check.progress", { done: size(done), all: size(all) }) : next}
        />
        <Figure label={t("storage.check.damage")} value={count(damage)} />
      </Figures>
      {damage > 0 && (
        <Signal tone="bad" still>
          {t("storage.check.damaged", many(damage))}
        </Signal>
      )}
    </>
  )
}
