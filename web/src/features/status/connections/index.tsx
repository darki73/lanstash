/**
 * Connections: what lanstash relays without caching: HTTPS connections (passed through to the
 * origin, never opened) and plain requests for hosts it does not cache. The tile shows the HTTPS
 * connections over the overview's range and those open now; the page shows them (cyan) and
 * pass-through requests a second (dashed) live while there are any, then both per bucket over the
 * range, the range's figures (trouble counted in the danger colour, nothing in faint ink) and,
 * folded away, the raw counters.
 */
import type { Kept } from "@/features/status/kept"
import { counter, peak, perSecond, series } from "@/features/status/model"
import { AllCounters, KeptChart, LiveStrip, stateOf } from "@/features/status/monitor"
import { spanOf } from "@/features/status/range"
import type { Live, TileArgs, ViewArgs } from "@/features/status/resource"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { Chart, type Trace } from "@/styled/chart"
import { Figure, Figures } from "@/styled/figures"
import { Tile } from "@/styled/tile"
import { View } from "@/styled/view"

/** The counters the page reads from what was kept. */
export const CONNECTIONS_KEPT = [
  "https.connections",
  "https.relayed",
  "https.refused",
  "https.bad_hello",
  "https.connect_failed",
  "pass-through.requests",
  "pass-through.bytes",
] as const

const active = (s: Live["snapshot"]) => counter(s, "https", "active")

const keptTraces = (k: Kept): Trace[] => [
  { values: k.values("https.connections"), tone: "origin" },
  { values: k.values("pass-through.requests"), tone: "plain", style: "dashed" },
]

export function ConnectionsTile({ live, href, current, kept }: TileArgs) {
  const t = useT("status")
  const s = live.snapshot
  const https = kept ? kept.total("https.connections") : counter(s, "https", "connections")
  return (
    <Tile
      name={t("resource.connections")}
      value={t("connections.tile.value", { n: count(https) })}
      detail={t("connections.tile.detail", { n: count(active(s)) })}
      tone="origin"
      href={href}
      current={current}
    >
      {kept && (
        <Chart
          size="small"
          window={kept.values("https.connections").length}
          label={t("connections.tile.chart")}
          traces={[{ values: kept.values("https.connections"), tone: "origin" }]}
        />
      )}
    </Tile>
  )
}

export function ConnectionsView({ live, kept }: ViewArgs) {
  const t = useT("status")
  const s = live.snapshot
  const open = series(live.recent, active)
  const passed = perSecond(live.recent, "pass-through", "requests")
  const k = (name: string) => (kept ? kept.total(name) : 0)
  const figure = (label: string, name: string, trouble = false) => (
    <Figure label={label} value={count(k(name))} state={stateOf(k(name), trouble)} />
  )
  return (
    <View
      title={t("resource.connections")}
      subtitle={t("connections.subtitle")}
      aside={kept ? t("range.last", { range: spanOf(t, kept.range) }) : undefined}
    >
      <LiveStrip
        live={live}
        label={t("connections.live")}
        traces={[
          { values: open, tone: "origin" },
          { values: passed, tone: "plain", style: "dashed" },
        ]}
        scale={count(Math.ceil(peak([...open, ...passed])))}
      >
        <Figure label={t("connections.httpsOpen")} value={count(active(s))} tone="origin" large />
        <Figure
          label={t("connections.passThroughNow")}
          value={`${(passed.at(-1) ?? 0).toFixed(1)}/s`}
          tone="plain"
          dashed
          large
        />
      </LiveStrip>
      <KeptChart kept={kept} label={t("connections.kept")} traces={keptTraces} unit={count} />
      {kept && (
        <Figures title={t("range.lastTitle", { range: spanOf(t, kept.range) })}>
          <Figure
            label={t("connections.httpsConnections")}
            value={count(k("https.connections"))}
            tone="origin"
            large
            state={stateOf(k("https.connections"))}
          />
          <Figure
            label={t("connections.passThroughRequests")}
            value={count(k("pass-through.requests"))}
            tone="plain"
            dashed
            large
            state={stateOf(k("pass-through.requests"))}
          />
          <Figure
            label={t("connections.passedThrough")}
            value={size(k("pass-through.bytes"))}
            large
            state={stateOf(k("pass-through.bytes"))}
          />
          <Figure
            label={t("connections.relayed")}
            value={count(k("https.relayed"))}
            large
            state={stateOf(k("https.relayed"))}
          />
          {figure(t("connections.refused"), "https.refused", true)}
          {figure(t("connections.badHello"), "https.bad_hello", true)}
          {figure(t("connections.connectFailed"), "https.connect_failed", true)}
        </Figures>
      )}
      <AllCounters snapshot={s} groups={["https", "pass-through"]} kept={kept} />
    </View>
  )
}
