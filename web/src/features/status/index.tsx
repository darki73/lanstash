/**
 * Overview: lanstash's first page, one range for all of it (the last day, week or month). Over
 * everything a status line says what lanstash is doing and holds the range's switch; under it a
 * row of tiles, one per resource (throughput, downloads, requests, storage, connections), each its
 * figure over the range and a small chart of it, linking to the resource's page. Then the home
 * (what lanstash saved, what moves now, the traffic, who downloads what, the stash) or the page of
 * the resource asked for. What lanstash kept over the range is read once for all of them.
 */
import { useState } from "react"
import type { ArtLink } from "@/contracts/art"
import type { HealthReader } from "@/contracts/health"
import { SERVED, type StatsReader, type TopReader } from "@/contracts/stats"
import { CONNECTIONS_KEPT, ConnectionsTile, ConnectionsView } from "@/features/status/connections"
import { counterNames } from "@/features/status/counters"
import { DOWNLOADS_KEPT, DownloadsTile, DownloadsView } from "@/features/status/downloads"
import { Home } from "@/features/status/home"
import { useKept, useTop } from "@/features/status/kept"
import { Pulse } from "@/features/status/pulse"
import type { Range } from "@/features/status/range"
import { REQUESTS_KEPT, RequestsTile, RequestsView } from "@/features/status/requests"
import { type Live, overviewLink, type ResourceId, type ViewArgs } from "@/features/status/resource"
import { STORAGE_KEPT, StorageTile, StorageView } from "@/features/status/storage"
import { THROUGHPUT_KEPT, ThroughputTile, ThroughputView } from "@/features/status/throughput"
import { useT } from "@/i18n"
import { Stack } from "@/styled/stack"
import { TileRow } from "@/styled/tile"

/** Every counter the overview reads from what was kept, once. */
export const KEPT_NAMES: readonly string[] = [
  ...new Set([...THROUGHPUT_KEPT, ...DOWNLOADS_KEPT, ...REQUESTS_KEPT, ...STORAGE_KEPT, ...CONNECTIONS_KEPT]),
]

/** The counter groups the resources' tables list. */
const COUNTER_GROUPS = ["requests", "downloads", "pass-through", "eviction", "https"] as const

function ResourceView({ resource, ...args }: ViewArgs & { readonly resource: ResourceId }) {
  switch (resource) {
    case "throughput":
      return <ThroughputView {...args} />
    case "downloads":
      return <DownloadsView {...args} />
    case "requests":
      return <RequestsView {...args} />
    case "storage":
      return <StorageView {...args} />
    case "connections":
      return <ConnectionsView {...args} />
  }
}

export interface OverviewProps {
  readonly live: Live
  /** The resource whose page is shown; none: the home. */
  readonly resource?: ResourceId
  /** Counters over time as lanstash kept them; none: nothing over a range. */
  readonly stats?: StatsReader
  /** The counters that went up most (the games sent most); none: not listed. */
  readonly top?: TopReader
  /** The checks lanstash makes of itself; none: the status line leaves them out. */
  readonly health?: HealthReader
  /** Where the checks' page is. */
  readonly healthHref?: string
  /** Where a game's icon is; letters stand in without it. */
  readonly art?: ArtLink
}

export function Overview({ live, resource, stats, top, health, healthHref = "/health", art }: OverviewProps) {
  const t = useT("status")
  const [range, setRange] = useState<Range>("day")
  const now = live.snapshot.now_ms
  const names = [...new Set([...KEPT_NAMES, ...counterNames(live.snapshot, COUNTER_GROUPS)])]
  const kept = useKept(stats, range, now, names)
  const served = useTop(top, SERVED, range, now)
  const tile = (id: ResourceId) => ({ live, kept, href: overviewLink(id), current: resource === id })
  return (
    <Stack wide>
      <Pulse
        snapshot={live.snapshot}
        kept={kept}
        health={health}
        healthHref={healthHref}
        range={range}
        onRange={setRange}
      />
      <TileRow label={t("resources")}>
        <ThroughputTile {...tile("throughput")} />
        <DownloadsTile {...tile("downloads")} />
        <RequestsTile {...tile("requests")} />
        <StorageTile {...tile("storage")} />
        <ConnectionsTile {...tile("connections")} />
      </TileRow>
      {resource === undefined ? (
        <Home live={live} kept={kept} served={served} art={art} />
      ) : (
        <ResourceView resource={resource} live={live} kept={kept} art={art} />
      )}
    </Stack>
  )
}
