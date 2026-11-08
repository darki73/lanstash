/**
 * Deleting panel: the last deletion of a game from the stash. While it runs: how much of what the
 * game held is gone (a meter, in bytes), how many pieces, how long it has run. Once done: how much
 * went and how long it took, and how many pieces could not be deleted, if any; with a link back
 * when given (the game's page has nothing left to show).
 */
import type { Deleting } from "@/contracts/stash"
import { useSpan, useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { Chip, Chips } from "@/styled/chips"
import { Meter } from "@/styled/meter"
import { Panel } from "@/styled/panel"
import { Signal } from "@/styled/signal"
import { Link } from "@/styled/text"

export interface DeletingPanelProps {
  readonly job: Deleting
  /** The game's name, for the title. */
  readonly name: string
  /** The time now (ms), for how long the job has run. */
  readonly now: number
  /** Where to go once it is done, and what to call it. */
  readonly back?: { readonly href: string; readonly label: string }
}

export function DeletingPanel({ job, name, now, back }: DeletingPanelProps) {
  const t = useT("platforms")
  const span = useSpan()
  const running = job.state === "running"
  const took = ((job.finished_ms ?? now) - job.started_ms) / 1000
  const title = job.partial ? (running ? "pruning" : "pruned") : running ? "deleting" : "deleted"
  return (
    <Panel
      title={t(`deleting.title.${title}`, { name })}
      action={
        <Signal tone={running ? "waiting" : job.failed > 0 ? "bad" : "good"} still={!running}>
          {t(running ? "deleting.running" : "deleting.done")}
        </Signal>
      }
    >
      <Meter
        fraction={job.bytes_total > 0 ? job.bytes_done / job.bytes_total : running ? 0 : 1}
        label={t("game.of", { have: size(job.bytes_done), want: size(job.bytes_total) })}
      />
      <Chips>
        <Chip value={`${count(job.items_done)}/${count(job.items_total)}`} label={t("deleting.pieces")} />
        <Chip value={span(took)} label={t(running ? "deleting.for" : "deleting.took")} />
        {job.failed > 0 && <Chip value={count(job.failed)} label={t("deleting.failed")} />}
      </Chips>
      {!running && back !== undefined && <Link href={back.href}>{back.label}</Link>}
    </Panel>
  )
}
