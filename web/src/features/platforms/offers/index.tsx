/**
 * Offers: the queue planner of a platform's games that are not in the cache (from its catalog, a
 * Steam library or Battle.net's products). They stand in a table: each with a box to choose it, its
 * name (leading to its page), its current version, what a prefill downloads (where the platform says)
 * and, where lancache's old cache was probed (Steam), about how much of it lancache holds; where the
 * games have more than one owner between them (several Steam accounts), who owns each. A search
 * narrows them by name (or id) as it is typed; they can be sorted by name, by size or by how much
 * lancache holds, and narrowed to the games lancache holds any of. A box over the table chooses all
 * the games shown, or none.
 *
 * Once games are chosen, a bar at the bottom says how many, what prefilling them downloads (and how
 * much of that lancache holds), what importing only what lancache holds would take, the free space on
 * the stash's disk (with a warning when the download is larger), and how many have no size. Its
 * buttons queue them, the largest first: Import from lancache (only what lancache holds), or Queue
 * prefills (lancache's old cache first where lanstash can read it, the CDN for the rest); then the
 * choice is cleared; Import from lancache leaves out the games lancache holds almost nothing of. Clear drops the choice.
 */
import { useState } from "react"
import { type ArtLink, noArt } from "@/contracts/art"
import { type Offer, ownersLine } from "@/contracts/catalog"
import { gameLink, type Platform } from "@/features/platforms/model"
import { usePrivacy } from "@/features/privacy"
import { useT, type Words } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { done } from "@/lib/percent"
import { ActionBar } from "@/styled/actionbar"
import { Button } from "@/styled/button"
import { Checkbox } from "@/styled/checkbox"
import { Panel } from "@/styled/panel"
import { PickCard, PickGrid } from "@/styled/pick"
import { Search } from "@/styled/search"
import { Segmented } from "@/styled/segmented"
import { Signal } from "@/styled/signal"
import { Cell, Row, Table } from "@/styled/table"
import { Link, Muted, Quiet } from "@/styled/text"
import { Toolbar } from "@/styled/toolbar"

export interface OffersProps {
  readonly platform: Platform
  readonly offers: readonly Offer[]
  /** Where a game's cover image is. */
  readonly art?: ArtLink
  /** Free bytes on the stash's disk, when lanstash measures it. */
  readonly free?: number
  /** Whether lanstash can read lancache's old cache (prefills take from it first). */
  readonly lancache?: boolean
  /** Queues prefills of `games` in this order: only what lancache holds, or a whole prefill. */
  readonly queue?: (games: readonly string[], from: "lancache-only" | "prefill") => void
}

type Order = "name" | "size" | "lancache"

const orders = ["name", "size", "lancache"] as const

const views = ["covers", "list"] as const

const filters = ["all", "held"] as const

/** How much of an offer lancache's old cache holds (an estimate), once probed. */
function inLancache(t: Words<"platforms">, o: Offer): string {
  const s = o.lancache
  if (s === undefined || s.want === 0) return "–"
  return s.have === 0 ? t("offers.heldNone") : t("offers.heldAbout", { share: done(s.have, s.want) })
}

const held = (o: Offer) => o.lancache?.have ?? 0

/**
 * Whether lancache holds enough of an offer to import: at least 100 MB and 1 % of it. An import reads
 * every one of its files' places in lancache's cache, so one that holds almost nothing takes long for
 * nothing.
 */
export const worthImporting = (o: Offer) => held(o) >= 1e8 && held(o) >= 0.01 * (o.download ?? o.lancache?.want ?? 0)

/** The offers whose name (or id) contains `query`, ignoring case. */
export function matching(offers: readonly Offer[], query: string): readonly Offer[] {
  const q = query.trim().toLowerCase()
  if (q === "") return offers
  return offers.filter((o) => o.name.toLowerCase().includes(q) || o.game.toLowerCase() === q)
}

/** The offers in `order`: by name, the largest download first, or the most in lancache first. */
export function sorted(offers: readonly Offer[], order: Order): Offer[] {
  const list = [...offers]
  if (order === "size") return list.sort((a, b) => (b.download ?? 0) - (a.download ?? 0))
  if (order === "lancache") return list.sort((a, b) => held(b) - held(a))
  return list.sort((a, b) => a.name.localeCompare(b.name))
}

/** What prefilling `chosen` takes: its downloads, how much of them lancache holds, and how many have no size. */
export function plan(chosen: readonly Offer[]) {
  return {
    download: chosen.reduce((a, o) => a + (o.download ?? 0), 0),
    inLancache: chosen.reduce((a, o) => a + held(o), 0),
    unsized: chosen.filter((o) => o.download === undefined).length,
  }
}

export function Offers({ platform: p, offers: listed, art = noArt, free, lancache = false, queue }: OffersProps) {
  const t = useT("platforms")
  // A masked game is left out: the library lists games by name.
  const privacy = usePrivacy()
  const offers = listed.filter((o) => privacy.of(p.id, o.game) !== "masked")
  const [query, setQuery] = useState("")
  const [order, setOrder] = useState<Order>("name")
  const [filter, setFilter] = useState<"all" | "held">("all")
  const [view, setView] = useState<"covers" | "list">("covers")
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set())
  if (offers.length === 0) {
    return <Quiet title={t("offers.allCached", { name: p.name })} />
  }
  const probed = offers.some((o) => o.lancache !== undefined)
  const ownedBy = ownersLine(offers)
  const shown = sorted(
    matching(offers, query).filter((o) => filter === "all" || held(o) > 0),
    order,
  )
  const picked = offers.filter((o) => chosen.has(o.game))
  const all = shown.length > 0 && shown.every((o) => chosen.has(o.game))
  const some = shown.some((o) => chosen.has(o.game))
  const choose = (games: readonly string[], on: boolean) => {
    const next = new Set(chosen)
    for (const g of games) {
      if (on) next.add(g)
      else next.delete(g)
    }
    setChosen(next)
  }
  const go = (from: "lancache-only" | "prefill") => {
    const list = from === "lancache-only" ? picked.filter(worthImporting) : picked
    const order = [...list].sort((a, b) =>
      from === "lancache-only" ? held(b) - held(a) : (b.download ?? 0) - (a.download ?? 0),
    )
    queue?.(
      order.map((o) => o.game),
      from,
    )
    setChosen(new Set())
  }
  const totals = plan(picked)
  const importable = picked.filter(worthImporting).length
  return (
    <>
      <Toolbar
        title={t("offers.title", { count: offers.length, n: count(offers.length), name: p.name })}
        tools={
          <>
            <Segmented
              label={t("offers.how")}
              options={views.map((id) => ({ id, label: t(`offers.view.${id}`) }))}
              value={view}
              onChange={setView}
            />
            {probed && (
              <Segmented
                label={t("games.which")}
                options={filters.map((id) => ({ id, label: t(`offers.filter.${id}`) }))}
                value={filter}
                onChange={setFilter}
              />
            )}
            <Segmented
              label={t("games.sort")}
              options={(probed ? orders : orders.filter((o) => o !== "lancache")).map((id) => ({
                id,
                label: t(`offers.order.${id}`),
              }))}
              value={order}
              onChange={setOrder}
            />
          </>
        }
      />
      <Search
        label={t("offers.search", { name: p.name })}
        placeholder={t("offers.searchPlaceholder")}
        value={query}
        onChange={setQuery}
      />
      {shown.length === 0 ? (
        <Quiet title={query.trim() === "" ? t("offers.none") : t("offers.noMatch", { query: query.trim() })} />
      ) : (
        <Panel
          title={t("offers.choose")}
          note={t("offers.shown", { n: count(shown.length) })}
          action={
            queue === undefined ? undefined : (
              <Checkbox
                checked={all}
                mixed={some}
                label={all ? t("offers.chooseNone") : t("offers.chooseAll", { n: count(shown.length) })}
                onChange={(on) =>
                  choose(
                    shown.map((o) => o.game),
                    on,
                  )
                }
              />
            )
          }
        >
          {view === "covers" ? (
            <PickGrid label={t("offers.grid", { name: p.name })}>
              {shown.map((o) => (
                <PickCard
                  key={o.game}
                  name={o.name}
                  href={gameLink(p.id, o.game)}
                  image={p.art === undefined ? undefined : art(p.id, p.art.by === "group" ? o.game : o.code)}
                  cover={p.art?.cover}
                  detail={[
                    o.download === undefined ? undefined : size(o.download),
                    t("game.version", { version: o.version }),
                    ownedBy(o),
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                  badge={
                    worthImporting(o) && o.lancache !== undefined ? (
                      <Signal tone="waiting" still>
                        {t("offers.heldBadge", { share: done(o.lancache.have, o.lancache.want) })}
                      </Signal>
                    ) : undefined
                  }
                  chosen={chosen.has(o.game)}
                  onChoose={queue === undefined ? undefined : (on) => choose([o.game], on)}
                />
              ))}
            </PickGrid>
          ) : (
            <Table
              columns={[
                t("offers.column.game"),
                t("offers.column.version"),
                t("offers.column.download"),
                ...(probed ? [t("offers.column.lancache")] : []),
                ...(shown.some((o) => ownedBy(o) !== undefined) ? [t("offers.column.owners")] : []),
              ]}
            >
              {shown.map((o) => (
                <Row key={o.game} tone={chosen.has(o.game) ? "strong" : "plain"}>
                  <Cell>
                    {queue !== undefined && (
                      <Checkbox
                        checked={chosen.has(o.game)}
                        label={t("offers.chooseOne", { name: o.name })}
                        onChange={(on) => choose([o.game], on)}
                      />
                    )}{" "}
                    <Link href={gameLink(p.id, o.game)}>{o.name}</Link>
                  </Cell>
                  <Cell muted>{o.version}</Cell>
                  <Cell>{o.download === undefined ? "–" : size(o.download)}</Cell>
                  {probed && <Cell muted={held(o) === 0}>{inLancache(t, o)}</Cell>}
                  {shown.some((x) => ownedBy(x) !== undefined) && <Cell muted>{ownedBy(o) ?? "–"}</Cell>}
                </Row>
              ))}
            </Table>
          )}
        </Panel>
      )}
      {queue !== undefined && picked.length > 0 && (
        <ActionBar
          label={t("offers.bar.label")}
          actions={
            <>
              <Button onClick={() => setChosen(new Set())}>{t("offers.bar.clear")}</Button>
              {importable > 0 && (
                <Button
                  onClick={() => go("lancache-only")}
                  label={t("offers.bar.importLabel", { n: count(importable) })}
                >
                  {t("offers.bar.import", { size: size(totals.inLancache) })}
                </Button>
              )}
              <Button
                onClick={() => go("prefill")}
                tone="accent"
                label={t("offers.bar.queueLabel", { n: count(picked.length) })}
              >
                {t("button.queuePrefills", { count: picked.length, n: count(picked.length) })}
              </Button>
            </>
          }
        >
          <Muted>
            {t("offers.bar.chosen", { count: picked.length, n: count(picked.length), size: size(totals.download) })}
            {lancache && totals.inLancache > 0 ? t("offers.bar.fromLancache", { size: size(totals.inLancache) }) : ""}
            {free === undefined ? "" : t("offers.bar.free", { size: size(free) })}
          </Muted>
          {totals.unsized > 0 && <Muted>{t("offers.bar.unsized", { n: count(totals.unsized), name: p.name })}</Muted>}
          {free !== undefined && totals.download > free && <Signal tone="bad">{t("offers.bar.tooBig")}</Signal>}
        </ActionBar>
      )}
    </>
  )
}
