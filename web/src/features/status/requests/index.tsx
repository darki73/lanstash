/**
 * Requests: what the clients ask for and how much of it the cache answers. The tile shows the hit
 * rate over the overview's range; the page shows requests a second (ink) and hits a second (amber)
 * live while clients ask, then requests and hits per bucket over the range, the range's figures
 * (trouble counted in the danger colour, nothing in faint ink) and, folded away, the raw counters.
 */
import type { Kept } from "@/features/status/kept"
import { counter, hitRates, peak, perSecond } from "@/features/status/model"
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
export const REQUESTS_KEPT = [
  "requests.requests",
  "requests.hits",
  "requests.misses",
  "requests.partial",
  "requests.errors",
  "requests.refused",
  "requests.retries",
  "requests.aborted",
  "requests.sendfile_bytes",
  "requests.copied_bytes",
] as const

/** The share of body bytes over the range the kernel sent from the file itself; undefined for none. */
export function keptSendfile(k: Kept): number | undefined {
  const sent = k.total("requests.sendfile_bytes")
  const all = sent + k.total("requests.copied_bytes")
  return all > 0 ? sent / all : undefined
}

const percent = (share: number | undefined) => (share === undefined ? "–" : `${(100 * share).toFixed(1)} %`)

const perSecondText = (v: number) => `${v < 10 ? v.toFixed(1) : v.toFixed(0)}/s`

/** The share of requests over the range the cache answered whole; undefined for none. */
export function keptHitRate(k: Kept): number | undefined {
  const hits = k.total("requests.hits")
  const misses = k.total("requests.misses")
  return hits + misses > 0 ? hits / (hits + misses) : undefined
}

const keptTraces = (k: Kept): Trace[] => [
  { values: k.values("requests.requests"), tone: "plain" },
  { values: k.values("requests.hits"), tone: "cache", style: "line" },
]

export function RequestsTile({ live, href, current, kept }: TileArgs) {
  const t = useT("status")
  const s = live.snapshot
  const rate = kept ? keptHitRate(kept) : hitRates(s).objects
  const requests = kept ? kept.total("requests.requests") : counter(s, "requests", "requests")
  return (
    <Tile
      name={t("resource.requests")}
      value={t("requests.tile.value", { rate: percent(rate) })}
      detail={t("requests.tile.detail", { n: count(requests) })}
      tone="cache"
      href={href}
      current={current}
    >
      {kept && (
        <Chart
          size="small"
          window={kept.values("requests.requests").length}
          label={t("requests.tile.chart")}
          traces={[
            { values: kept.values("requests.requests"), tone: "plain" },
            { values: kept.values("requests.hits"), tone: "cache", style: "line" },
          ]}
        />
      )}
    </Tile>
  )
}

export function RequestsView({ live, kept }: ViewArgs) {
  const t = useT("status")
  const s = live.snapshot
  const requests = perSecond(live.recent, "requests", "requests")
  const hits = perSecond(live.recent, "requests", "hits")
  const k = (name: string) => (kept ? kept.total(`requests.${name}`) : 0)
  const rate = kept ? keptHitRate(kept) : undefined
  const figure = (label: string, name: string, trouble = false) => (
    <Figure label={label} value={count(k(name))} state={stateOf(k(name), trouble)} />
  )
  return (
    <View
      title={t("resource.requests")}
      subtitle={t("requests.subtitle")}
      aside={kept ? t("range.last", { range: spanOf(t, kept.range) }) : undefined}
    >
      <LiveStrip
        live={live}
        label={t("requests.live")}
        traces={[
          { values: requests, tone: "plain" },
          { values: hits, tone: "cache", style: "line" },
        ]}
        scale={perSecondText(peak(requests))}
      >
        <Figure label={t("requests.requestsNow")} value={perSecondText(requests.at(-1) ?? 0)} tone="plain" large />
        <Figure label={t("requests.hitsNow")} value={perSecondText(hits.at(-1) ?? 0)} tone="cache" large />
      </LiveStrip>
      <KeptChart kept={kept} label={t("requests.kept")} traces={keptTraces} unit={count} />
      {kept && (
        <Figures title={t("range.lastTitle", { range: spanOf(t, kept.range) })}>
          <Figure
            label={t("requests.hitRate")}
            value={percent(rate)}
            tone="cache"
            large
            state={rate === undefined ? "quiet" : undefined}
          />
          <Figure
            label={t("requests.requests")}
            value={count(k("requests"))}
            tone="plain"
            large
            state={stateOf(k("requests"))}
          />
          <Figure label={t("requests.hits")} value={count(k("hits"))} large state={stateOf(k("hits"))} />
          <Figure label={t("requests.misses")} value={count(k("misses"))} large state={stateOf(k("misses"))} />
          {figure(t("requests.partial"), "partial")}
          {figure(t("requests.retries"), "retries", true)}
          {figure(t("requests.errors"), "errors", true)}
          {figure(t("requests.refused"), "refused", true)}
          {figure(t("requests.aborted"), "aborted")}
          {keptSendfile(kept) !== undefined && (
            <Figure label={t("requests.sendfile")} value={percent(keptSendfile(kept))} />
          )}
        </Figures>
      )}
      <AllCounters snapshot={s} groups={["requests"]} kept={kept} />
    </View>
  )
}
