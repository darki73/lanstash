/**
 * Home: the overview's first page, what lanstash did and does, from the top down:
 *   - what it saved over the range (Lifetime);
 *   - the bytes moving now, live, and who is downloading what, while anyone is;
 *   - the traffic over the range, per bucket;
 *   - the games sent most over the range, beside what the stash holds: how full it is and its
 *     largest games.
 */
import type { ArtLink } from "@/contracts/art"
import { nowRows } from "@/features/clients"
import { dirGame } from "@/features/platforms/model"
import type { Kept } from "@/features/status/kept"
import { Lifetime } from "@/features/status/lifetime"
import { many, peak, rate, series, shownName, throughput } from "@/features/status/model"
import { KeptChart, LiveStrip } from "@/features/status/monitor"
import { spanOf } from "@/features/status/range"
import type { Live } from "@/features/status/resource"
import { StashPanel } from "@/features/status/stash"
import { MostServed } from "@/features/status/traffic"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { rate as mbps } from "@/lib/rate"
import { BarGroup, BarRow } from "@/styled/bars"
import { initialsOf } from "@/styled/cards"
import type { Trace } from "@/styled/chart"
import { Figure } from "@/styled/figures"
import { Monogram } from "@/styled/monogram"
import { Panel, PanelGrid } from "@/styled/panel"

export interface HomeProps {
  readonly live: Live
  readonly kept: Kept | null | undefined
  /** The counters of the games sent most over the range. */
  readonly served: readonly { readonly name: string; readonly value: number }[]
  readonly art?: ArtLink
}

/** Who downloads what now: drawn only while someone does (the status line says when nobody does). */
function Now({ live, art }: { readonly live: Live; readonly art?: ArtLink }) {
  const t = useT("status")
  const tCommon = useT("common")
  const hidden = tCommon("hiddenGame")
  const s = live.snapshot
  const rows = nowRows(s.clients ?? [], (p, d) => dirGame(s.usage, p, d, tCommon("launcherData")))
  const fastest = Math.max(1, ...rows.map((r) => r.perSec))
  if (rows.length === 0) return null
  return (
    <Panel title={t("home.now.title")} note={t("home.now.games", many(rows.length))}>
      <BarGroup title={t("home.now.clients")} value={mbps(rows.reduce((n, r) => n + r.perSec, 0) / 1e6)}>
        {rows.map((r) => {
          const got = r.hit + r.miss
          const game = shownName(r.game.name, hidden)
          return (
            <BarRow
              key={r.key}
              icon={
                <Monogram
                  letters={initialsOf(game) || "?"}
                  image={r.game.art?.kinds.icon ? art?.(r.service, r.game.art.id, "icon") : undefined}
                />
              }
              name={r.client.label ?? r.client.host ?? r.client.address}
              detail={game}
              value={mbps(r.perSec / 1e6)}
              sub={t("home.now.soFar", { size: size(r.session) })}
              fraction={r.perSec / fastest}
              tone="cache"
              split={{ share: got > 0 ? r.hit / got : 1, rest: "origin" }}
              href={r.game.href}
            />
          )
        })}
      </BarGroup>
    </Panel>
  )
}

const keptTraces = (k: Kept): Trace[] => [
  { values: k.values("requests.hit_bytes"), tone: "cache" },
  { values: k.values("downloads.fetched_bytes"), tone: "origin" },
  { values: k.values("requests.miss_bytes"), tone: "plain", style: "dashed" },
]

export function Home({ live, kept, served, art }: HomeProps) {
  const t = useT("status")
  const launcher = useT("common")("launcherData")
  const s = live.snapshot
  const through = throughput(s)
  const now: Trace[] = [
    { values: series(live.recent, (x) => rate(x, "requests.hit_bytes")), tone: "cache" },
    { values: series(live.recent, (x) => rate(x, "downloads.fetched_bytes")), tone: "origin" },
    { values: series(live.recent, (x) => rate(x, "requests.miss_bytes")), tone: "plain", style: "dashed" },
  ]
  const words = spanOf(t, kept ? kept.range : "day")
  return (
    <>
      <Lifetime snapshot={s} kept={kept} />
      <LiveStrip live={live} label={t("throughput.live")} traces={now} scale={mbps(peak(now.flatMap((x) => x.values)))}>
        <Figure label={t("throughput.fromCache")} value={mbps(through.fromCache)} tone="cache" large />
        <Figure label={t("throughput.downloading")} value={mbps(through.downloading)} tone="plain" dashed large />
        <Figure label={t("throughput.fromInternet")} value={mbps(through.fromInternet)} tone="origin" large />
      </LiveStrip>
      <Now live={live} art={art} />
      <Panel title={t("home.traffic")} note={t("range.last", { range: words })}>
        <KeptChart kept={kept} label={t("throughput.kept")} traces={keptTraces} unit={size} />
      </Panel>
      <PanelGrid wide>
        <MostServed counters={served} gameOf={(p, d) => dirGame(s.usage, p, d, launcher)} words={words} art={art} />
        <StashPanel snapshot={s} art={art} />
      </PanelGrid>
    </>
  )
}
