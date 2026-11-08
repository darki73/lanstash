/**
 * Go to: the palette that goes anywhere in the app by typing (Ctrl+K, or / from anywhere). It finds:
 *   - pages: the overview and each of its resources, the settings, each platform and each part of
 *     its page (Steam · Library);
 *   - games in the cache, on every platform: with their cover (a platform's icon where it has
 *     icons), their platform and their size;
 *   - games of a platform's library not in the cache yet, with their download size where known:
 *     their page offers to prefill them.
 * A game is found by its name, its id or a store code (initials too: "gta"); the best answers come
 * first, the larger game first among equals. With nothing typed it offers the games used last,
 * then the overview, the settings and each platform that holds something.
 */
import type { ArtLink } from "@/contracts/art"
import { type CatalogByPlatform, notCached } from "@/contracts/catalog"
import type { Snapshot } from "@/contracts/status"
import { partsOf } from "@/features/platforms"
import { type Game, gameLink, type Platform, platforms, type Section, sectionLink } from "@/features/platforms/model"
import { usePrivacy } from "@/features/privacy"
import { overviewLink, type ResourceId, resources } from "@/features/status/resource"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { match } from "@/lib/match"
import { Brand } from "@/styled/brand"
import { Glyph } from "@/styled/glyph"
import { Monogram } from "@/styled/monogram"
import { Palette, type PaletteGroup, type PaletteItem } from "@/styled/palette"

/** How many rows each group shows at most. */
const most = { pages: 6, games: 12, library: 8, recent: 6 } as const

export interface GoToProps {
  readonly snapshot: Snapshot
  /** Each platform's library, once the API said it. */
  readonly catalogs?: CatalogByPlatform
  readonly art?: ArtLink
}

const mark = (p: Platform) => (p.brand === undefined ? <Monogram letters={p.letters} /> : <Brand name={p.brand} />)

/** A thing that may answer: its row, the names it is found by, and how large it is. */
interface Candidate {
  readonly item: PaletteItem
  readonly names: readonly string[]
  readonly bytes: number
}

/** The best of the candidates that answer, best first (larger first among equals). */
function best(all: readonly Candidate[], typed: string, limit: number): PaletteItem[] {
  return all
    .flatMap((c) => {
      const scores = c.names.map((n) => match(n, typed)).filter((s) => s !== undefined)
      return scores.length === 0 ? [] : [{ c, score: Math.max(...scores) }]
    })
    .sort((a, b) => b.score - a.score || b.c.bytes - a.c.bytes)
    .slice(0, limit)
    .map(({ c }) => c.item)
}

/** The names pages are found by, in the reader's language. */
interface PageWords {
  readonly overview: string
  readonly settings: string
  readonly resource: (r: ResourceId) => string
  readonly part: (s: Section) => string
  /** A platform's part: "Steam Library". */
  readonly platformPart: (platform: string, part: string) => string
}

function pages(list: readonly Platform[], w: PageWords): Candidate[] {
  const page = (key: string, title: string, href: string, icon: PaletteItem["icon"], sub?: string): Candidate => ({
    item: { key, title, href, icon, ...(sub === undefined ? {} : { sub }) },
    names: [title],
    bytes: 0,
  })
  return [
    page("overview", w.overview, "/overview", <Glyph name="overview" />),
    ...resources.map((r) =>
      page(`overview/${r}`, w.resource(r), overviewLink(r), <Glyph name="overview" />, w.overview),
    ),
    page("settings", w.settings, "/settings", <Glyph name="settings" />),
    ...list.flatMap((p) => [
      { ...page(`platform/${p.id}`, p.name, sectionLink(p.id, "cached"), mark(p)), bytes: p.bytes },
      ...partsOf(p)
        .filter((s) => s !== "cached")
        .map((s) => ({
          ...page(`platform/${p.id}/${s}`, w.platformPart(p.name, w.part(s)), sectionLink(p.id, s), mark(p)),
          names: [w.platformPart(p.name, w.part(s)), w.part(s)],
        })),
    ]),
  ]
}

function gameItem(p: Platform, g: Game, art: ArtLink | undefined): PaletteItem {
  const square = p.art?.icon === true
  const image = g.art === undefined ? undefined : art?.(p.id, g.art, square ? "icon" : "header")
  return {
    key: `game/${p.id}/${g.key}`,
    title: g.name,
    sub: p.name,
    meta: size(g.bytes),
    href: g.id === undefined ? sectionLink(p.id, "cached") : gameLink(p.id, g.id),
    icon: mark(p),
    ...(image === undefined ? {} : { image, square }),
  }
}

export function GoTo({ snapshot, catalogs = {}, art }: GoToProps) {
  const privacy = usePrivacy()
  const t = useT("palette")
  const tFrame = useT("frame")
  const tStatus = useT("status")
  const tPlatforms = useT("platforms")
  const words: PageWords = {
    overview: tFrame("nav.overview"),
    settings: tFrame("nav.settings"),
    resource: (r) => tStatus(`resource.${r}`),
    part: (s) => tPlatforms(`part.${s}`),
    platformPart: (platform, part) => t("part", { platform, part }),
  }
  const find = (typed: string): PaletteGroup[] => {
    const list = platforms(snapshot.usage)
    // A masked game is never offered: finding it by its name would name it.
    const games = list.flatMap((p) =>
      p.games
        .filter((g) => g.privacy !== "masked")
        .map((g) => ({
          p,
          g,
          c: { item: gameItem(p, g, art), names: [g.name, g.id ?? "", ...g.codes], bytes: g.bytes },
        })),
    )
    if (typed.trim() === "") {
      // The pages one starts from: the overview, the settings and each platform holding something.
      const top = pages(
        list.filter((p) => p.bytes > 0),
        words,
      ).filter((c) => !c.item.key.startsWith("overview/") && c.item.key.split("/").length <= 2)
      return [
        {
          title: t("group.recent"),
          items: [...games]
            .filter(({ g }) => g.lastUsedMs > 0)
            .sort((a, b) => b.g.lastUsedMs - a.g.lastUsedMs)
            .slice(0, most.recent)
            .map(({ c }) => c.item),
        },
        { title: t("group.pages"), items: top.map((c) => c.item) },
      ]
    }
    const library = list.flatMap((p) => {
      const cached = new Set(p.games.flatMap((g) => [...(g.id === undefined ? [] : [g.id]), ...g.codes]))
      const shown = notCached(catalogs[p.id], cached).filter((o) => privacy.of(p.id, o.game) !== "masked")
      return shown.map((o) => {
        const image = p.art === undefined ? undefined : art?.(p.id, o.game, p.art.icon ? "icon" : "header")
        return {
          item: {
            key: `library/${p.id}/${o.game}`,
            title: o.name,
            sub: t("notCached", { platform: p.name }),
            ...(o.download === undefined ? {} : { meta: size(o.download) }),
            href: gameLink(p.id, o.game),
            icon: mark(p),
            ...(image === undefined ? {} : { image, square: p.art?.icon === true }),
          },
          names: [o.name, o.game, o.code],
          bytes: o.download ?? 0,
        }
      })
    })
    return [
      {
        title: t("group.games"),
        items: best(
          games.map(({ c }) => c),
          typed,
          most.games,
        ),
      },
      { title: t("group.library"), items: best(library, typed, most.library) },
      { title: t("group.pages"), items: best(pages(list, words), typed, most.pages) },
    ]
  }
  return <Palette label={t("label")} placeholder={t("placeholder")} find={find} none={t("none")} />
}
