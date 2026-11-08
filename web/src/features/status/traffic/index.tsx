/**
 * Traffic: the games sent most to clients over the overview's range, from the counters lanstash
 * keeps per store directory and minute, added up per game (a Steam app's depots are one game),
 * most first, each with its icon and a link to its page.
 */
import type { ArtLink } from "@/contracts/art"
import { servedGame } from "@/contracts/stats"
import type { DirGame } from "@/features/platforms/model"
import { shownName } from "@/features/status/model"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { BarGroup, BarRow } from "@/styled/bars"
import { initialsOf } from "@/styled/cards"
import { Monogram } from "@/styled/monogram"
import { Panel } from "@/styled/panel"
import { Quiet } from "@/styled/text"

/** How many games the list shows. */
const TOP = 8

export type GameOf = (platform: string, game: string) => DirGame

export interface Served {
  readonly key: string
  readonly service: string
  readonly game: DirGame
  readonly value: number
}

/** The games sent most: the counters of store directories added up per game, most first, at most TOP. */
export function mostServed(
  counters: readonly { readonly name: string; readonly value: number }[],
  gameOf: GameOf,
): Served[] {
  const games = new Map<string, Served>()
  for (const c of counters) {
    const of = servedGame(c.name)
    if (of === undefined) continue
    const game = gameOf(of.platform, of.game)
    const key = `${of.platform}/${game.id}`
    const had = games.get(key)
    games.set(key, { key, service: of.platform, game, value: (had?.value ?? 0) + c.value })
  }
  return [...games.values()].sort((a, b) => b.value - a.value).slice(0, TOP)
}

export interface MostServedProps {
  readonly counters: readonly { readonly name: string; readonly value: number }[]
  readonly gameOf: GameOf
  /** The range in words ("24 hours"). */
  readonly words: string
  readonly art?: ArtLink
}

export function MostServed({ counters, gameOf, words, art }: MostServedProps) {
  const t = useT("status")
  const hidden = useT("common")("hiddenGame")
  const games = mostServed(counters, gameOf)
  const largest = Math.max(1, ...games.map((g) => g.value))
  return (
    <Panel title={t("traffic.title")} note={t("range.last", { range: words })}>
      {games.length === 0 ? (
        <Quiet title={t("traffic.empty.title")} hint={t("traffic.empty.hint", { range: words })} />
      ) : (
        <BarGroup title={t("traffic.games")} value={size(games.reduce((n, g) => n + g.value, 0))}>
          {games.map((g) => (
            <BarRow
              key={g.key}
              icon={
                <Monogram
                  letters={initialsOf(shownName(g.game.name, hidden)) || "?"}
                  image={g.game.art?.kinds.icon ? art?.(g.service, g.game.art.id, "icon") : undefined}
                />
              }
              name={shownName(g.game.name, hidden)}
              detail={g.game.platform}
              value={size(g.value)}
              fraction={g.value / largest}
              tone="cache"
              href={g.game.href}
            />
          ))}
        </BarGroup>
      )}
    </Panel>
  )
}
