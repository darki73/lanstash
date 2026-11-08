/**
 * Downloads: what lanstash fetches from the internet. The tile shows the downloads finished over
 * the overview's range and what is in flight now; the page shows the objects in flight live while
 * there are any, then the downloads started per bucket over the range (failures in the danger
 * colour), the range's figures (trouble counted in that colour, nothing in faint ink), the list of
 * what is in flight and, folded away, the raw counters.
 */
import { DownloadList } from "@/features/status/downloads/list"
import type { Kept } from "@/features/status/kept"
import { counter, objectsInFlight, peak, series } from "@/features/status/model"
import { AllCounters, KeptChart, LiveStrip, stateOf } from "@/features/status/monitor"
import { spanOf } from "@/features/status/range"
import type { TileArgs, ViewArgs } from "@/features/status/resource"
import { useT } from "@/i18n"
import { count } from "@/lib/count"
import { Chart, type Trace } from "@/styled/chart"
import { Figure, Figures } from "@/styled/figures"
import { Tile } from "@/styled/tile"
import { View } from "@/styled/view"

/** The counters the page reads from what was kept. */
export const DOWNLOADS_KEPT = [
  "downloads.started",
  "downloads.completed",
  "downloads.failed",
  "downloads.joined",
  "downloads.crc_mismatch",
  "downloads.store_errors",
  "downloads.range_ignored",
  "downloads.upstream_4xx",
  "downloads.upstream_5xx",
  "downloads.timeouts",
  "downloads.answer_wait_ms",
  "downloads.answers_timed",
] as const

/** The mean wait for the origin's first answer over the range, in ms; none without one timed. */
export function firstByte(kept: Kept): number | undefined {
  const timed = kept.total("downloads.answers_timed")
  return timed > 0 ? kept.total("downloads.answer_wait_ms") / timed : undefined
}

const keptTraces = (k: Kept): Trace[] => [
  { values: k.values("downloads.started"), tone: "origin" },
  { values: k.values("downloads.failed"), tone: "plain", style: "dashed" },
]

export function DownloadsTile({ live, href, current, kept }: TileArgs) {
  const t = useT("status")
  const s = live.snapshot
  const failed = kept ? kept.total("downloads.failed") : counter(s, "downloads", "failed")
  const done = kept ? kept.total("downloads.completed") : counter(s, "downloads", "completed")
  return (
    <Tile
      name={t("resource.downloads")}
      value={t("downloads.tile.value", { n: count(done) })}
      detail={t("downloads.tile.detail", { inFlight: count(objectsInFlight(s)), failed: count(failed) })}
      tone="origin"
      href={href}
      current={current}
    >
      {kept && (
        <Chart
          size="small"
          window={kept.values("downloads.started").length}
          label={t("downloads.tile.chart")}
          traces={[{ values: kept.values("downloads.started"), tone: "origin" }]}
        />
      )}
    </Tile>
  )
}

export function DownloadsView({ live, kept }: ViewArgs) {
  const t = useT("status")
  const s = live.snapshot
  const inFlight = series(live.recent, objectsInFlight)
  const k = (name: string) => (kept ? kept.total(`downloads.${name}`) : 0)
  const wait = kept ? firstByte(kept) : undefined
  const figure = (label: string, name: string, trouble = false, large = false) => (
    <Figure label={label} value={count(k(name))} large={large} state={stateOf(k(name), trouble)} />
  )
  return (
    <View
      title={t("resource.downloads")}
      subtitle={t("downloads.subtitle")}
      aside={kept ? t("range.last", { range: spanOf(t, kept.range) }) : undefined}
    >
      <LiveStrip
        live={live}
        label={t("downloads.live")}
        traces={[{ values: inFlight, tone: "origin" }]}
        scale={t("downloads.scale", { n: count(peak(inFlight)) })}
      >
        <Figure label={t("downloads.inFlight")} value={count(objectsInFlight(s))} tone="origin" large />
        <Figure label={t("downloads.pieces")} value={count(s.downloads.length)} large />
      </LiveStrip>
      {s.downloads.length > 0 && <DownloadList downloads={s.downloads} />}
      <KeptChart kept={kept} label={t("downloads.kept")} traces={keptTraces} unit={count} />
      {kept && (
        <Figures title={t("range.lastTitle", { range: spanOf(t, kept.range) })}>
          {figure(t("downloads.started"), "started", false, true)}
          {figure(t("downloads.completed"), "completed", false, true)}
          {figure(t("downloads.failed"), "failed", true, true)}
          {figure(t("downloads.joined"), "joined", false, true)}
          {figure(t("downloads.crcMismatch"), "crc_mismatch", true)}
          {figure(t("downloads.storeErrors"), "store_errors", true)}
          {figure(t("downloads.rangeIgnored"), "range_ignored", true)}
          {figure(t("downloads.upstream4xx"), "upstream_4xx", true)}
          {figure(t("downloads.upstream5xx"), "upstream_5xx", true)}
          {figure(t("downloads.timeouts"), "timeouts", true)}
          {wait !== undefined && (
            <Figure label={t("downloads.firstByte")} value={t("downloads.ms", { ms: Math.round(wait) })} />
          )}
        </Figures>
      )}
      <AllCounters snapshot={s} groups={["downloads"]} kept={kept} />
    </View>
  )
}
