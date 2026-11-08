/**
 * Runs: a platform's prefill jobs that ended, newest first, as lanstash kept them (they outlive a
 * restart): the game (a link to its page), how it ended, what it downloaded and what the stash
 * already held, how long it took and when it ended. Read when the part opens and again whenever
 * a job ends.
 */
import { useEffect, useState } from "react"
import type { PrefillRun, RunsReader } from "@/contracts/runs"
import { took } from "@/contracts/runs"
import { useAgo, useSpan, useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { Panel } from "@/styled/panel"
import { Signal, type SignalTone } from "@/styled/signal"
import { Cell, Row, Table } from "@/styled/table"
import { Link, Quiet } from "@/styled/text"

export interface RunsPanelProps {
  readonly platform: string
  readonly runs: RunsReader
  /** When the platform's last job ended (ms): a new one is read again. */
  readonly ended: number | null
  readonly now: number
  readonly nameOf: (game: string) => string
  readonly linkOf: (game: string) => string
}

const tones: Readonly<Record<PrefillRun["job"]["state"], SignalTone>> = {
  done: "good",
  stopped: "waiting",
  failed: "bad",
  running: "waiting",
}

export function RunsPanel({ platform, runs, ended, now, nameOf, linkOf }: RunsPanelProps) {
  const ago = useAgo()
  const span = useSpan()
  const t = useT("platforms")
  const [kept, setKept] = useState<readonly PrefillRun[] | undefined | null>(null)
  // biome-ignore lint/correctness/useExhaustiveDependencies: a job that ended is asked for again.
  useEffect(() => {
    let live = true
    void runs(platform).then((r) => {
      if (live) setKept(r)
    })
    return () => {
      live = false
    }
  }, [platform, runs, ended])
  if (kept === null) return null
  return (
    <Panel title={t("runs.title")} note={kept === undefined ? undefined : t("runs.kept", { n: kept.length })}>
      {kept === undefined || kept.length === 0 ? (
        <Quiet title={t("runs.none.title")} hint={t("runs.none.hint")} />
      ) : (
        <Table
          columns={[
            t("runs.column.game"),
            t("runs.column.ended"),
            t("runs.column.downloaded"),
            t("runs.column.held"),
            t("runs.column.took"),
            t("runs.column.when"),
          ]}
        >
          {kept.map(({ job }) => {
            const length = took(job)
            return (
              <Row key={`${job.game}-${job.started_ms}`}>
                <Cell>
                  <Link href={linkOf(job.game)}>{nameOf(job.game)}</Link>
                </Cell>
                <Cell>
                  <Signal tone={tones[job.state]} still title={job.error ?? t(`runs.state.${job.state}`)}>
                    {t(`runs.state.${job.state}`)}
                  </Signal>
                </Cell>
                <Cell>{size(job.bytes)}</Cell>
                <Cell muted>{job.bytes_skipped === undefined ? "–" : size(job.bytes_skipped)}</Cell>
                <Cell muted>{length === undefined ? "–" : span(Math.round(length / 1000))}</Cell>
                <Cell muted>{job.finished_ms === null ? "–" : ago(now, job.finished_ms)}</Cell>
              </Row>
            )
          })}
        </Table>
      )}
    </Panel>
  )
}
