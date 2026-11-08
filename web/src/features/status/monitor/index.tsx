/**
 * Monitor: the parts every resource of the overview draws the same way.
 *   - LiveStrip: the last two minutes, live, as a low chart with the figures of now beside it,
 *     drawn only while something moved in them (an idle resource shows no flat line).
 *   - KeptChart: the range the overview shows, as lanstash kept it, with a burst cut at the top
 *     so it does not flatten the rest; a quiet note while it is read or when nothing is kept.
 *   - AllCounters: the raw counters, over the range and since lanstash started, folded away until
 *     asked for.
 */
import type { ReactNode } from "react"
import type { Snapshot } from "@/contracts/status"
import { Counters } from "@/features/status/counters"
import type { Kept } from "@/features/status/kept"
import { per, spanOf } from "@/features/status/range"
import { type Live, secondsPerSnapshot } from "@/features/status/resource"
import { useSpan, useT } from "@/i18n"
import { Chart, capped, type Trace } from "@/styled/chart"
import { Figures } from "@/styled/figures"
import { Fold } from "@/styled/fold"
import { Panel } from "@/styled/panel"
import { Stack } from "@/styled/stack"
import { Quiet } from "@/styled/text"

/** How many of the newest values say whether a trace is moving. */
const RECENT = 15

/** Whether any trace rose above 0 in its newest values. */
export function moving(traces: readonly Trace[]): boolean {
  return traces.some((t) => t.values.slice(-RECENT).some((v) => v > 0))
}

export interface LiveStripProps {
  readonly live: Live
  readonly label: string
  readonly traces: readonly Trace[]
  /** Words for the top of the scale. */
  readonly scale: string
  /** The figures of now. */
  readonly children?: ReactNode
}

export function LiveStrip({ live, label, traces, scale, children }: LiveStripProps) {
  const t = useT("status")
  const span = useSpan()
  if (!moving(traces)) return null
  return (
    <Panel title={t("monitor.live")} note={t("range.last", { range: span(live.keep * secondsPerSnapshot) })}>
      <Stack wide>
        <Chart size="medium" traces={traces} window={live.keep} label={label} scale={scale} />
        <Figures>{children}</Figures>
      </Stack>
    </Panel>
  )
}

export interface KeptChartProps {
  readonly kept: Kept | null | undefined
  readonly label: string
  /** The traces, from what was kept. */
  readonly traces: (kept: Kept) => readonly Trace[]
  /** A value per bucket in words ("1.2 GB"). */
  readonly unit: (value: number) => string
}

export function KeptChart({ kept, label, traces, unit }: KeptChartProps) {
  const t = useT("status")
  if (kept === undefined) return <Quiet title={t("monitor.reading")} />
  if (kept === null) {
    return <Quiet title={t("monitor.nothingKept.title")} hint={t("monitor.nothingKept.hint")} />
  }
  const shown = traces(kept)
  const values = shown.flatMap((x) => x.values)
  const top = capped(values)
  const highest = Math.max(0, ...values)
  const bucket = per(kept.spec.step, t)
  const range = spanOf(t, kept.range)
  return (
    <Chart
      traces={shown}
      window={Math.max(1, ...shown.map((x) => x.values.length))}
      label={t("monitor.chart", { label, bucket, range })}
      top={top}
      scale={
        top === undefined
          ? t("monitor.scale", { value: unit(highest), bucket })
          : t("monitor.scaleCut", { top: unit(top), highest: unit(highest) })
      }
      span={range}
    />
  )
}

export interface AllCountersProps {
  readonly snapshot: Snapshot
  readonly groups: readonly string[]
  readonly kept: Kept | null | undefined
}

export function AllCounters({ snapshot, groups, kept }: AllCountersProps) {
  const t = useT("status")
  return (
    <Fold title={t("monitor.allCounters")} summary={groups.join(" · ")}>
      <Counters snapshot={snapshot} groups={groups} kept={kept} />
    </Fold>
  )
}

/** A figure's state: quiet when it is 0, an alarm when it counts trouble above 0. */
export function stateOf(value: number, trouble = false): "quiet" | "alarm" | undefined {
  if (value === 0) return "quiet"
  return trouble ? "alarm" : undefined
}
