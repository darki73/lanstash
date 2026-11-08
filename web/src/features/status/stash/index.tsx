/**
 * Stash: what the stash holds, by game rather than by store directory (a Steam app's depots are one
 * game; a launcher's own data is one row), the largest first, each game linking to its page.
 *   - StashPanel (the overview's home): how full the store is, split by platform, and the largest
 *     games of all platforms, with a link to the storage page.
 *   - Library (the storage page): per platform its largest games (as many for each, so the
 *     columns line up), its launcher data said in its heading, and a link to the platform's page
 *     for all of them.
 */
import type { ArtLink } from "@/contracts/art"
import type { Snapshot } from "@/contracts/status"
import { type Game, gameLink, type Platform, platformLink, platforms } from "@/features/platforms/model"
import { many, storage } from "@/features/status/model"
import { overviewLink } from "@/features/status/resource"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { BarGroup, BarRow } from "@/styled/bars"
import { Brand } from "@/styled/brand"
import { Capacity } from "@/styled/capacity"
import { initialsOf } from "@/styled/cards"
import { Monogram } from "@/styled/monogram"
import { Panel, PanelGrid } from "@/styled/panel"
import { Stack } from "@/styled/stack"
import { Link, Quiet } from "@/styled/text"

/** How many games the home lists, and how many each platform lists on the storage page. */
const HOME_GAMES = 8
const PLATFORM_GAMES = 12

const held = (snapshot: Snapshot) => platforms(snapshot.usage).filter((p) => p.bytes > 0)

const mark = (p: Platform) => (p.brand === undefined ? <Monogram letters={p.letters} /> : <Brand name={p.brand} />)

/** A game's row: its icon, name (and its platform's, unless `named` is false), size and bar. */
function GameRow(props: { p: Platform; g: Game; largest: number; art?: ArtLink; named?: boolean }) {
  const { p, g, largest, art, named = true } = props
  const hidden = useT("common")("hiddenGame")
  const name = g.privacy === "masked" ? hidden : g.name
  const image = g.art !== undefined && p.art?.icon ? art?.(p.id, g.art, "icon") : undefined
  return (
    <BarRow
      icon={<Monogram letters={initialsOf(name) || "?"} image={image} />}
      name={name}
      detail={named ? p.name : undefined}
      value={size(g.bytes)}
      fraction={g.bytes / largest}
      tone="cache"
      href={g.privacy === "masked" ? undefined : gameLink(p.id, g.id ?? g.codes[0] ?? g.key)}
    />
  )
}

/** How full the store is: each platform's part, and what is free when lanstash measures it. */
export function Fullness({ snapshot }: { readonly snapshot: Snapshot }) {
  const t = useT("status")
  const st = storage(snapshot)
  const whole = st.stored + (st.free ?? 0)
  return (
    <Capacity
      label={t("stash.fullness.label")}
      held={t("stash.fullness.stored", { size: size(st.stored) })}
      free={st.free === undefined ? undefined : t("stash.fullness.free", { free: size(st.free), whole: size(whole) })}
      parts={held(snapshot).map((p) => ({
        key: p.id,
        icon: mark(p),
        label: p.name,
        value: size(p.bytes),
        share: whole > 0 ? p.bytes / whole : 0,
      }))}
    />
  )
}

export function StashPanel({ snapshot, art }: { readonly snapshot: Snapshot; readonly art?: ArtLink }) {
  const t = useT("status")
  const all = held(snapshot)
  const games = all.flatMap((p) => p.games.map((g) => ({ p, g }))).sort((a, b) => b.g.bytes - a.g.bytes)
  const largest = Math.max(1, ...games.map(({ g }) => g.bytes))
  return (
    <Panel title={t("stash.title")} note={t("stash.games", many(games.length))}>
      {all.length === 0 ? (
        <Quiet title={t("stash.empty.title")} hint={t("stash.empty.hint")} />
      ) : (
        <Stack wide>
          <Fullness snapshot={snapshot} />
          <BarGroup
            title={t("stash.largest")}
            value={t("stash.shownOf", { shown: HOME_GAMES, all: count(games.length) })}
            more={<Link href={overviewLink("storage")}>{t("stash.allByPlatform")}</Link>}
          >
            {games.slice(0, HOME_GAMES).map(({ p, g }) => (
              <GameRow key={`${p.id}/${g.key}`} p={p} g={g} largest={largest} art={art} />
            ))}
          </BarGroup>
        </Stack>
      )}
    </Panel>
  )
}

export function Library({ snapshot, art }: { readonly snapshot: Snapshot; readonly art?: ArtLink }) {
  const t = useT("status")
  const all = held(snapshot)
  const largest = Math.max(1, ...all.flatMap((p) => p.games.map((g) => g.bytes)))
  return (
    <Panel title={t("stash.library")}>
      {all.length === 0 ? (
        <Quiet title={t("stash.empty.title")} hint={t("stash.empty.hint")} />
      ) : (
        <PanelGrid wide>
          {all.map((p) => {
            const launcher = p.systemData.reduce((n, r) => n + r.bytes, 0)
            return (
              <BarGroup
                key={p.id}
                title={p.name}
                value={size(p.bytes)}
                detail={
                  launcher > 0
                    ? t("stash.gamesLauncher", { ...many(p.games.length), size: size(launcher) })
                    : t("stash.games", many(p.games.length))
                }
                more={
                  p.games.length > PLATFORM_GAMES ? (
                    <Link href={platformLink(p.id)}>
                      {t("stash.allOn", { n: count(p.games.length), platform: p.name })}
                    </Link>
                  ) : undefined
                }
              >
                {p.games.slice(0, PLATFORM_GAMES).map((g) => (
                  <GameRow key={g.key} p={p} g={g} largest={largest} art={art} named={false} />
                ))}
              </BarGroup>
            )
          })}
        </PanelGrid>
      )}
    </Panel>
  )
}
