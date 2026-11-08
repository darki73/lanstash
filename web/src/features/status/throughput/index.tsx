/**
 * Throughput: bytes to clients and from the internet. The tile shows what clients got over the
 * overview's range and what they get now; the page shows the last two minutes live while bytes
 * move, then the range as lanstash kept it: per bucket, what clients got from the cache (amber),
 * what they got while lanstash downloaded it (dashed) and what lanstash pulled from the internet
 * (cyan), with the range's totals under it.
 */
import type { Kept } from "@/features/status/kept"
import { peak, rate, series, throughput } from "@/features/status/model"
import { AllCounters, KeptChart, LiveStrip, stateOf } from "@/features/status/monitor"
import { spanOf } from "@/features/status/range"
import type { Live, TileArgs, ViewArgs } from "@/features/status/resource"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { rate as mbps } from "@/lib/rate"
import { Chart, type Trace } from "@/styled/chart"
import { Figure, Figures } from "@/styled/figures"
import { Tile } from "@/styled/tile"
import { View } from "@/styled/view"

/** The counters the page reads from what was kept. */
export const THROUGHPUT_KEPT = ["requests.hit_bytes", "requests.miss_bytes", "downloads.fetched_bytes"] as const
const [HIT, MISS, FETCHED] = THROUGHPUT_KEPT

function liveTraces(live: Live): Trace[] {
  return [
    { values: series(live.recent, (s) => rate(s, "requests.hit_bytes")), tone: "cache" },
    { values: series(live.recent, (s) => rate(s, "downloads.fetched_bytes")), tone: "origin" },
    { values: series(live.recent, (s) => rate(s, "requests.miss_bytes")), tone: "plain", style: "dashed" },
  ]
}

const keptTraces = (k: Kept): Trace[] => [
  { values: k.values(HIT), tone: "cache" },
  { values: k.values(FETCHED), tone: "origin" },
  { values: k.values(MISS), tone: "plain", style: "dashed" },
]

/** Of what clients got over the range, the share that came from the cache; undefined for none. */
export function cacheShare(k: Kept): number | undefined {
  const all = k.total(HIT) + k.total(MISS)
  return all > 0 ? k.total(HIT) / all : undefined
}

export function ThroughputTile({ live, href, current, kept }: TileArgs) {
  const t = useT("status")
  const now = throughput(live.snapshot)
  return (
    <Tile
      name={t("resource.throughput")}
      value={
        kept
          ? t("throughput.tile.value", { size: size(kept.total(HIT) + kept.total(MISS)) })
          : mbps(now.fromCache + now.downloading)
      }
      detail={t("throughput.tile.detail", { rate: mbps(now.fromCache + now.downloading) })}
      tone="cache"
      href={href}
      current={current}
    >
      {kept && (
        <Chart
          size="small"
          window={kept.values(HIT).length}
          label={t("throughput.tile.chart")}
          traces={[
            { values: kept.values(HIT).map((v, i) => v + (kept.values(MISS)[i] ?? 0)), tone: "cache" },
            { values: kept.values(FETCHED), tone: "origin", style: "line" },
          ]}
        />
      )}
    </Tile>
  )
}

export function ThroughputView({ live, kept }: ViewArgs) {
  const t = useT("status")
  const s = live.snapshot
  const through = throughput(s)
  const now = liveTraces(live)
  const toClients = (now[0]?.values ?? []).map((v, i) => v + (now[2]?.values[i] ?? 0))
  const share = kept ? cacheShare(kept) : undefined
  const total = (name: string) => (kept ? kept.total(name) : 0)
  return (
    <View
      title={t("resource.throughput")}
      subtitle={t("throughput.subtitle")}
      aside={kept ? t("range.last", { range: spanOf(t, kept.range) }) : undefined}
    >
      <LiveStrip
        live={live}
        label={t("throughput.live")}
        traces={now}
        scale={mbps(peak([...toClients, ...(now[1]?.values ?? [])]))}
      >
        <Figure label={t("throughput.fromCache")} value={mbps(through.fromCache)} tone="cache" large />
        <Figure label={t("throughput.downloading")} value={mbps(through.downloading)} tone="plain" dashed large />
        <Figure label={t("throughput.fromInternet")} value={mbps(through.fromInternet)} tone="origin" large />
        <Figure label={t("throughput.peak")} value={mbps(peak(toClients))} large />
      </LiveStrip>
      <KeptChart kept={kept} label={t("throughput.kept")} traces={keptTraces} unit={size} />
      {kept && (
        <Figures title={t("range.lastTitle", { range: spanOf(t, kept.range) })}>
          <Figure
            label={t("throughput.fromCache")}
            value={size(total(HIT))}
            tone="cache"
            large
            state={stateOf(total(HIT))}
          />
          <Figure
            label={t("throughput.downloading")}
            value={size(total(MISS))}
            tone="plain"
            dashed
            large
            state={stateOf(total(MISS))}
          />
          <Figure
            label={t("throughput.fromInternet")}
            value={size(total(FETCHED))}
            tone="origin"
            large
            state={stateOf(total(FETCHED))}
          />
          <Figure
            label={t("throughput.servedFromCache")}
            value={share === undefined ? "–" : `${Math.round(share * 100)} %`}
            large
            state={share === undefined ? "quiet" : undefined}
          />
        </Figures>
      )}
      <AllCounters snapshot={s} groups={["requests", "downloads", "pass-through"]} kept={kept} />
    </View>
  )
}
