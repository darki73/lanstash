/**
 * Storage: what the store holds and how busy its disk is. The tile shows the bytes stored and
 * traces what was written over the overview's range; the page shows the writes live while there
 * are any, how full the store is by platform, the bytes written (amber) and freed by eviction
 * (dashed) per bucket over the range, the range's figures, the background store check when it is
 * on, the library by game and, folded away, the raw counters.
 */
import type { Kept } from "@/features/status/kept"
import { peak, rate, series, storage } from "@/features/status/model"
import { AllCounters, KeptChart, LiveStrip, stateOf } from "@/features/status/monitor"
import { spanOf } from "@/features/status/range"
import type { TileArgs, ViewArgs } from "@/features/status/resource"
import { Fullness, Library } from "@/features/status/stash"
import { Check } from "@/features/status/storage/check"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { rate as mbps } from "@/lib/rate"
import { Chart, type Trace } from "@/styled/chart"
import { Figure, Figures } from "@/styled/figures"
import { Panel } from "@/styled/panel"
import { Tile } from "@/styled/tile"
import { View } from "@/styled/view"

/** The counters the page reads from what was kept. */
export const STORAGE_KEPT = [
  "downloads.stored_bytes",
  "eviction.evicted_bytes",
  "eviction.evicted_segments",
  "eviction.evicted_files",
  "eviction.journals_compacted",
  "eviction.failed",
] as const

const written = (s: ViewArgs["live"]["snapshot"]) => rate(s, "downloads.stored_bytes")
const evicted = (s: ViewArgs["live"]["snapshot"]) => rate(s, "eviction.evicted_bytes")

const keptTraces = (k: Kept): Trace[] => [
  { values: k.values("downloads.stored_bytes"), tone: "cache" },
  { values: k.values("eviction.evicted_bytes"), tone: "plain", style: "dashed" },
]

export function StorageTile({ live, href, current, kept }: TileArgs) {
  const t = useT("status")
  const st = storage(live.snapshot)
  return (
    <Tile
      name={t("resource.storage")}
      value={size(st.stored)}
      detail={
        st.free === undefined
          ? t("storage.tile.entries", { n: count(live.snapshot.usage.length) })
          : t("storage.tile.free", { size: size(st.free) })
      }
      tone="cache"
      href={href}
      current={current}
    >
      {kept && (
        <Chart
          size="small"
          window={kept.values("downloads.stored_bytes").length}
          label={t("storage.tile.chart")}
          traces={[{ values: kept.values("downloads.stored_bytes"), tone: "cache" }]}
        />
      )}
    </Tile>
  )
}

export function StorageView({ live, kept, art }: ViewArgs) {
  const t = useT("status")
  const s = live.snapshot
  const st = storage(s)
  const writes = series(live.recent, written)
  const frees = series(live.recent, evicted)
  const k = (name: string) => (kept ? kept.total(name) : 0)
  return (
    <View
      title={t("resource.storage")}
      subtitle={t("storage.subtitle")}
      aside={kept ? t("range.last", { range: spanOf(t, kept.range) }) : undefined}
    >
      <LiveStrip
        live={live}
        label={t("storage.live")}
        traces={[
          { values: writes, tone: "cache" },
          { values: frees, tone: "plain", style: "dashed" },
        ]}
        scale={mbps(peak([...writes, ...frees]))}
      >
        <Figure label={t("storage.writing")} value={mbps(written(s))} tone="cache" large />
        <Figure label={t("storage.freeing")} value={mbps(evicted(s))} tone="plain" dashed large />
      </LiveStrip>
      <Panel title={t("storage.howFull")}>
        <Fullness snapshot={s} />
      </Panel>
      <KeptChart kept={kept} label={t("storage.kept")} traces={keptTraces} unit={size} />
      {kept && (
        <Figures title={t("range.lastTitle", { range: spanOf(t, kept.range) })}>
          <Figure
            label={t("storage.written")}
            value={size(k("downloads.stored_bytes"))}
            tone="cache"
            large
            state={stateOf(k("downloads.stored_bytes"))}
          />
          <Figure
            label={t("storage.evicted")}
            value={size(k("eviction.evicted_bytes"))}
            tone="plain"
            dashed
            large
            state={stateOf(k("eviction.evicted_bytes"))}
          />
          <Figure label={t("storage.storedNow")} value={size(st.stored)} large />
          <Figure label={t("storage.freeNow")} value={st.free === undefined ? "–" : size(st.free)} large />
          <Figure
            label={t("storage.segmentsEvicted")}
            value={count(k("eviction.evicted_segments"))}
            state={stateOf(k("eviction.evicted_segments"))}
          />
          <Figure
            label={t("storage.filesEvicted")}
            value={count(k("eviction.evicted_files"))}
            state={stateOf(k("eviction.evicted_files"))}
          />
          <Figure
            label={t("storage.journalsCompacted")}
            value={count(k("eviction.journals_compacted"))}
            state={stateOf(k("eviction.journals_compacted"))}
          />
          <Figure
            label={t("storage.evictionFailures")}
            value={count(k("eviction.failed"))}
            state={stateOf(k("eviction.failed"), true)}
          />
        </Figures>
      )}
      <Check snapshot={s} />
      <Library snapshot={s} art={art} />
      <AllCounters snapshot={s} groups={["storage", "eviction"]} kept={kept} />
    </View>
  )
}
