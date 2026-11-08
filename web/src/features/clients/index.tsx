/**
 * Clients: who downloads what through the cache. On top, the clients downloading now (from every
 * status snapshot), each client and game drawn like a launcher's download: the game's art behind
 * it, the client and platform over the game's name, how fast over the last seconds, what it got in
 * this download and when that started (a client that got nothing of the game for two minutes
 * starts a new one), how much of the last seconds came from the cache, a little graph of the last
 * moments (cache in amber, internet in cyan), and a bar for the share from the cache; with nobody
 * downloading, a line says so. Under it, over the last day, week or month (as lanstash kept it by
 * the minute): a bar of each client's share of all that went to clients, then a card per client
 * across the page, the one that got most first (the busiest dozen, then all on asking), in a colour
 * of its own: on its left what it got and how that divides between the cache and the internet, on
 * its right its games with their icons, largest first, each bar split the same way. A search finds
 * clients by name, address or a game they got (for a network of many PCs). A client is called by the name given
 * it, else what the network calls it, else its address; each card names it anew (an empty name
 * takes the given one away). A store directory is shown as its game (a Steam app's depots are one
 * game), as the stash knows it. The range is read again a few seconds after each minute ends (when
 * lanstash has kept it).
 */
import { useEffect, useState } from "react"
import type { ArtLink } from "@/contracts/art"
import {
  type ClientId,
  type ClientNamer,
  type ClientNow,
  type ClientsReader,
  type ClientTotals,
  cacheShare,
  clientName,
} from "@/contracts/clients"
import type { Snapshot } from "@/contracts/status"
import type { DirGame } from "@/features/platforms/model"
import { type Range, rangeOptions, ranges } from "@/features/status/range"
import { useAgo, useT, type Words } from "@/i18n"
import { size } from "@/lib/bytes"
import { fold, match } from "@/lib/match"
import { BarGroup, BarRow } from "@/styled/bars"
import { Button } from "@/styled/button"
import { Capacity, type CapacityPart } from "@/styled/capacity"
import { initialsOf } from "@/styled/cards"
import { Download, type Moment } from "@/styled/download"
import { TextInput } from "@/styled/field"
import { Machine } from "@/styled/machine"
import { Monogram } from "@/styled/monogram"
import { Panel } from "@/styled/panel"
import { Portion } from "@/styled/portion"
import { Search } from "@/styled/search"
import { Segmented } from "@/styled/segmented"
import { Stack } from "@/styled/stack"
import { Link, Muted, Quiet } from "@/styled/text"
import { Toolbar } from "@/styled/toolbar"
import { View } from "@/styled/view"

/** A store directory's game, as the stash knows it. */
export type GameOf = (platform: string, dir: string) => DirGame

/** A client getting a game now: its directories added up. */
export interface NowRow {
  readonly key: string
  readonly client: ClientId
  readonly service: string
  readonly game: DirGame
  readonly perSec: number
  /** Of the last seconds: from the cache, and from the internet. */
  readonly hit: number
  readonly miss: number
  /** What it got in this download (since lanstash started, from an older one), and since when. */
  readonly session: number
  readonly since: number | undefined
}

/** The clients downloading now, per client and game (directories added up), fastest first. */
export function nowRows(clients: readonly ClientNow[], gameOf: GameOf): NowRow[] {
  const rows = new Map<string, NowRow>()
  for (const c of clients) {
    const game = gameOf(c.service, c.game)
    const key = `${c.address}/${c.service}/${game.id}`
    const had = rows.get(key)
    rows.set(key, {
      key,
      client: c,
      service: c.service,
      game,
      perSec: (had?.perSec ?? 0) + c.per_sec,
      hit: (had?.hit ?? 0) + c.hit_bytes,
      miss: (had?.miss ?? 0) + c.miss_bytes,
      session: (had?.session ?? 0) + (c.session_bytes ?? c.total_bytes),
      since:
        c.since_ms === undefined ? had?.since : had?.since === undefined ? c.since_ms : Math.min(had.since, c.since_ms),
    })
  }
  return [...rows.values()].sort(
    (a, b) => b.perSec - a.perSec || clientName(a.client).localeCompare(clientName(b.client)),
  )
}

/** A row's speed in each recent snapshot, split between the cache and the internet, oldest first. */
export function momentsOf(key: string, recent: readonly Snapshot[], gameOf: GameOf): Moment[] {
  return recent.map((s) => {
    const r = nowRows(s.clients ?? [], gameOf).find((x) => x.key === key)
    if (r === undefined) return {}
    const share = cacheShare(r.hit, r.miss) ?? 1
    return { cache: r.perSec * share, origin: r.perSec * (1 - share) }
  })
}

/** What a client got of each game over a range: directories added up per game, largest first. */
export interface GameRow {
  readonly key: string
  readonly service: string
  readonly game: DirGame
  readonly hit: number
  readonly miss: number
}

/** A client's games over a range: directories added up per game, largest first. */
export function gamesOf(client: ClientTotals, gameOf: GameOf): GameRow[] {
  const games = new Map<string, GameRow>()
  for (const g of client.games) {
    const game = gameOf(g.service, g.game)
    const key = `${g.service}/${game.id}`
    const had = games.get(key)
    games.set(key, { key, service: g.service, game, hit: (had?.hit ?? 0) + g.hit, miss: (had?.miss ?? 0) + g.miss })
  }
  return [...games.values()].sort((a, b) => b.hit + b.miss - (a.hit + a.miss))
}

/** The clients that got any of a game's directories (`mine`), with what they got, most first. */
export function gotIt(
  clients: readonly ClientTotals[],
  mine: (service: string, dir: string) => boolean,
): { client: ClientTotals; hit: number; miss: number }[] {
  return clients
    .map((client) => {
      const games = client.games.filter((g) => mine(g.service, g.game))
      return {
        client,
        hit: games.reduce((n, g) => n + g.hit, 0),
        miss: games.reduce((n, g) => n + g.miss, 0),
      }
    })
    .filter((c) => c.hit + c.miss > 0)
    .sort((a, b) => b.hit + b.miss - (a.hit + a.miss))
}

/** How many client cards show before "Show all" (a network of many PCs lists the busiest first). */
const CARDS = 12

/** How many clients the share bar names; the rest are one part. */
const SHARES = 5

/**
 * The clients that answer what was typed: by name given, network name, address or a game they got
 * (case, accents and punctuation do not count; "10.0.1" finds 10.0.1.5); all of them for nothing typed.
 */
export function findClients(list: readonly ClientTotals[], typed: string, gameOf: GameOf): ClientTotals[] {
  if (fold(typed) === "") return [...list]
  return list.filter((c) =>
    [c.label, c.host, c.address, ...c.games.map((g) => gameOf(g.service, g.game).name)].some(
      (s) => s !== null && match(s, typed) !== undefined,
    ),
  )
}

/** Each client's share of what all of them got: the largest few by name, the rest as one. */
export function shares(list: readonly ClientTotals[], t: Words<"clients">): CapacityPart[] {
  const total = list.reduce((n, c) => n + c.hit + c.miss, 0)
  if (total === 0) return []
  const sorted = [...list].sort((a, b) => b.hit + b.miss - (a.hit + a.miss))
  const named = sorted.slice(0, SHARES).map((c) => ({
    key: c.address,
    label: clientName(c),
    value: size(c.hit + c.miss),
    share: (c.hit + c.miss) / total,
  }))
  const rest = sorted.slice(SHARES).reduce((n, c) => n + c.hit + c.miss, 0)
  return rest > 0
    ? [
        ...named,
        {
          key: "rest",
          label: t("most.more", { count: sorted.length - SHARES }),
          value: size(rest),
          share: rest / total,
        },
      ]
    : named
}

/** A game's wide picture (a Steam banner; a Battle.net launcher background, its cover being a poster). */
const wideArt = (art: ArtLink, service: string, game: DirGame) =>
  game.art === undefined
    ? undefined
    : art(service, game.art.id, game.art.kinds.cover === "wide" ? "header" : "background")

/** A game's square icon, where the platform has them. */
const iconArt = (art: ArtLink, service: string, game: DirGame) =>
  game.art?.kinds.icon ? art(service, game.art.id, "icon") : undefined

/** A share as whole percent, or a dash when there is nothing to share. */
const percent = (share: number | undefined) => (share === undefined ? "–" : `${Math.round(share * 100)} %`)

/** Who a client is besides its name: its address, and the network's name when a given one hides it. */
const besides = (c: ClientId) => (c.label !== null && c.host !== null ? `${c.address} · ${c.host}` : c.address)

/** How far back a game's page looks for who got it. */
const GOT_IT_SPAN = 30 * 24 * 3_600_000

/**
 * Who got a game over the last 30 days: each client, what it got and how much came from the
 * cache. Nothing is drawn while it is read, nor when nobody got it.
 */
export function WhoGotIt(props: {
  readonly clients: ClientsReader
  /** Whether a store directory of a service is one of the game's. */
  readonly mine: (service: string, dir: string) => boolean
  readonly at: number
}) {
  const { clients, mine, at } = props
  const [got, setGot] = useState<readonly ClientTotals[]>()
  const to = Math.floor(at / 3_600_000) * 3_600_000
  useEffect(() => {
    let live = true
    void clients(to - GOT_IT_SPAN, to + 3_600_000).then((c) => {
      if (live) setGot(c)
    })
    return () => {
      live = false
    }
  }, [clients, to])
  const t = useT("clients")
  const rows = gotIt(got ?? [], mine)
  if (rows.length === 0) return null
  const largest = Math.max(1, ...rows.map((r) => r.hit + r.miss))
  return (
    <Panel title={t("whoGotIt.title")} note={t("whoGotIt.note")}>
      <BarGroup title={t("whoGotIt.clients")} value={`${rows.length}`}>
        {rows.map((r) => (
          <BarRow
            key={r.client.address}
            icon={<Monogram letters={initialsOf(clientName(r.client)) || "?"} />}
            name={clientName(r.client)}
            detail={r.client.address}
            value={size(r.hit + r.miss)}
            sub={t("cache", { percent: percent(cacheShare(r.hit, r.miss)) })}
            fraction={(r.hit + r.miss) / largest}
            tone="cache"
            split={{ share: cacheShare(r.hit, r.miss) ?? 1, rest: "origin" }}
          />
        ))}
      </BarGroup>
    </Panel>
  )
}

export interface ClientsViewProps {
  /** The clients downloading now (the snapshot's; absent from an older lanstash). */
  readonly now: readonly ClientNow[] | undefined
  /** The latest snapshots, oldest first: the graph of each download now (none: no graph). */
  readonly recent?: readonly Snapshot[]
  readonly gameOf: GameOf
  readonly clients: ClientsReader
  /** Names clients; none: no renaming. */
  readonly name?: ClientNamer
  /** Where a game's art is (none: no art). */
  readonly art?: ArtLink
  /** Now (ms): the end of the range. */
  readonly at: number
}

/** The download bands: one per client and game getting it now. */
function NowBands(props: {
  rows: readonly NowRow[]
  recent: readonly Snapshot[]
  gameOf: GameOf
  art?: ArtLink
  at: number
}) {
  const { rows, recent, gameOf, art, at } = props
  const ago = useAgo()
  const t = useT("clients")
  return (
    <Stack wide>
      {rows.map((r) => {
        const share = cacheShare(r.hit, r.miss)
        return (
          <Download
            key={r.key}
            title={r.game.name}
            doing={t("now.doing", { client: clientName(r.client), platform: r.game.platform })}
            image={art === undefined ? undefined : wideArt(art, r.service, r.game)}
            readings={[
              { label: t("now.speed"), value: t("now.perSec", { size: size(r.perSec) }) },
              { label: t("now.session"), value: size(r.session) },
              { label: t("now.started"), value: r.since === undefined ? "–" : ago(at, r.since) },
              { label: t("now.fromCache"), value: percent(share), tone: "cache" },
            ]}
            moments={momentsOf(r.key, recent, gameOf)}
            fraction={share ?? 0}
            state={t("now.state")}
            percent={percent(share)}
            barTone="cache"
            action={r.game.href === undefined ? undefined : <Link href={r.game.href}>{t("now.game")}</Link>}
          />
        )
      })}
    </Stack>
  )
}

export function ClientsView({ now, recent = [], gameOf, clients, name, art, at }: ClientsViewProps) {
  const t = useT("clients")
  const [range, setRange] = useState<Range>("day")
  const [kept, setKept] = useState<{ range: Range; clients: readonly ClientTotals[] | undefined }>()
  const [tick, setTick] = useState(0)
  const [editing, setEditing] = useState<{ address: string; label: string; error?: string }>()
  // Read again 5 s after each minute ends: lanstash keeps a minute as it ends.
  const minute = Math.floor((at - 5_000) / 60_000)
  // biome-ignore lint/correctness/useExhaustiveDependencies: minute and tick ask again.
  useEffect(() => {
    let live = true
    const to = (minute + 1) * 60_000
    void clients(to - ranges[range].span, to).then((c) => {
      if (live) setKept({ range, clients: c })
    })
    return () => {
      live = false
    }
  }, [range, clients, minute, tick])
  const [query, setQuery] = useState("")
  const [all, setAll] = useState(false)
  const rows = nowRows(now ?? [], gameOf)
  const shown = kept?.range === range ? kept : undefined
  const found = findClients(shown?.clients ?? [], query, gameOf)
  const listed = all || query !== "" ? found : found.slice(0, CARDS)
  const save = async () => {
    if (editing === undefined || name === undefined) return
    const error = await name(editing.address, editing.label)
    if (error !== undefined) setEditing({ ...editing, error })
    else {
      setEditing(undefined)
      setTick((n) => n + 1)
    }
  }
  return (
    <View
      title={t("title")}
      subtitle={t("subtitle")}
      aside={rows.length === 0 ? undefined : t("downloadingNow", { count: rows.length })}
    >
      <Stack wide>
        <Toolbar
          title={t("now.title")}
          tools={
            now === undefined ? (
              <Muted>{t("now.notCounted")}</Muted>
            ) : rows.length === 0 ? (
              <Muted>{t("now.nobody")}</Muted>
            ) : undefined
          }
        />
        {rows.length > 0 && <NowBands rows={rows} recent={recent} gameOf={gameOf} art={art} at={at} />}
        <Toolbar
          title={t(`over.${range}`)}
          tools={
            <>
              <Search
                label={t("find.label")}
                placeholder={t("find.placeholder")}
                value={query}
                onChange={setQuery}
                compact
              />
              <Segmented
                label={t("range.label")}
                options={rangeOptions.map((o) => ({ id: o.id, label: t(`range.${o.id}`) }))}
                value={range}
                onChange={setRange}
              />
            </>
          }
        />
        {shown === undefined ? (
          <Quiet title={t("reading")} />
        ) : shown.clients === undefined ? (
          <Quiet title={t("notKept.title")} hint={t("notKept.hint")} />
        ) : shown.clients.length === 0 ? (
          <Quiet title={t("nobody.title")} hint={t(`nobody.${range}`)} />
        ) : (
          <Stack wide>
            {query === "" && (
              <Panel title={t("most.title")} note={t("most.clients", { count: shown.clients.length })}>
                <Capacity
                  label={t("most.label")}
                  held={t("most.held", { size: size(shown.clients.reduce((n, c) => n + c.hit + c.miss, 0)) })}
                  parts={shares(shown.clients, t)}
                />
              </Panel>
            )}
            {found.length === 0 && <Quiet title={t("find.none")} hint={t("find.noneHint")} />}
            {listed.map((c) => {
              const games = gamesOf(c, gameOf)
              const largest = Math.max(1, ...games.map((g) => g.hit + g.miss))
              const mine = editing?.address === c.address ? editing : undefined
              return (
                <Machine
                  key={c.address}
                  name={clientName(c)}
                  detail={besides(c)}
                  action={
                    name !== undefined && mine === undefined ? (
                      <Button onClick={() => setEditing({ address: c.address, label: c.label ?? "" })}>
                        {t("card.rename")}
                      </Button>
                    ) : undefined
                  }
                  editing={
                    mine === undefined ? undefined : (
                      <Toolbar
                        title={mine.error ?? t("card.nameIt")}
                        tools={
                          <>
                            <TextInput
                              label={t("card.nameOf", { address: c.address })}
                              value={mine.label}
                              placeholder={c.host ?? c.address}
                              onChange={(label) => setEditing({ address: c.address, label })}
                              onEnter={() => void save()}
                              onEscape={() => setEditing(undefined)}
                            />
                            <Button tone="accent" onClick={() => void save()}>
                              {t("card.save")}
                            </Button>
                            <Button onClick={() => setEditing(undefined)}>{t("card.cancel")}</Button>
                          </>
                        }
                      />
                    )
                  }
                  side={
                    <Portion
                      value={size(c.hit + c.miss)}
                      caption={t(`card.got.${range}`)}
                      parts={[
                        {
                          label: t("card.fromStash"),
                          value: `${size(c.hit)} · ${percent(cacheShare(c.hit, c.miss))}`,
                          share: cacheShare(c.hit, c.miss) ?? 0,
                          tone: "cache",
                        },
                        {
                          label: t("card.fromInternet"),
                          value: size(c.miss),
                          share: 1 - (cacheShare(c.hit, c.miss) ?? 1),
                          tone: "origin",
                        },
                      ]}
                    />
                  }
                >
                  <BarGroup title={t("card.games")} value={`${games.length}`}>
                    {games.map((g) => (
                      <BarRow
                        key={g.key}
                        icon={
                          <Monogram
                            letters={initialsOf(g.game.name) || "?"}
                            image={art === undefined ? undefined : iconArt(art, g.service, g.game)}
                          />
                        }
                        name={g.game.name}
                        detail={g.game.platform}
                        value={size(g.hit + g.miss)}
                        sub={t("cache", { percent: percent(cacheShare(g.hit, g.miss)) })}
                        fraction={(g.hit + g.miss) / largest}
                        tone="cache"
                        split={{ share: cacheShare(g.hit, g.miss) ?? 1, rest: "origin" }}
                      />
                    ))}
                  </BarGroup>
                </Machine>
              )
            })}
            {found.length > listed.length && (
              <Button onClick={() => setAll(true)}>{t("card.showAll", { count: found.length })}</Button>
            )}
          </Stack>
        )}
      </Stack>
    </View>
  )
}
