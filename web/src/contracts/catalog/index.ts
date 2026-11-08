/**
 * Catalog: a platform's games, from GET /api/v1/catalog/<platform>: every Battle.net game there is
 * (its product code, its name, its directory on the CDN, the id lanstash knows it by, and current
 * version once lanstash looked them up: known), and the Steam accounts' games (the app id as code
 * and game, with the download size and the accounts that own it). notCached picks the games a
 * platform page offers to prefill: looked up, with a version, not in the cache, one per directory;
 * ownersLine says who owns one, where the offers have more than one owner between them.
 */

export interface CatalogProduct {
  readonly code: string
  readonly name: string | null
  /** Its directory on the CDN (Hero-Live-a, sc1live); null until looked up, or when it has none. */
  readonly game: string | null
  readonly version: string | null
  readonly known: boolean
  /** Bytes a prefill downloads, where the platform says (Steam). */
  readonly download?: number
  /** How much of it lancache's old cache holds (bytes; an estimate), once probed (Steam). */
  readonly lancache?: { readonly have: number; readonly want: number }
  /** The accounts that own it, where the platform signs in to several (Steam). */
  readonly owners?: readonly string[]
  /** The operating systems it installs for (`windows`, `macos`, `linux`), when more than one (Steam). */
  readonly systems?: readonly string[]
}

export interface Catalog {
  readonly products: readonly CatalogProduct[]
}

/** A product that can be prefilled: it has a directory, a version and a name. */
export interface Offer {
  readonly code: string
  readonly name: string
  readonly game: string
  readonly version: string
  readonly download?: number
  readonly lancache?: { readonly have: number; readonly want: number }
  readonly owners?: readonly string[]
}

/** Each platform's catalog, by platform id, once the API said it. */
export type CatalogByPlatform = Readonly<Record<string, Catalog>>

/** The products not in `cached` (directories), one per directory, by name. */
export function notCached(catalog: Catalog | undefined, cached: ReadonlySet<string>): Offer[] {
  const seen = new Set<string>()
  const out: Offer[] = []
  for (const p of catalog?.products ?? []) {
    if (p.game === null || p.version === null || p.name === null || cached.has(p.game) || seen.has(p.game)) continue
    seen.add(p.game)
    out.push({
      code: p.code,
      name: p.name,
      game: p.game,
      version: p.version,
      ...(p.download === undefined ? {} : { download: p.download }),
      ...(p.lancache === undefined ? {} : { lancache: p.lancache }),
      ...(p.owners === undefined ? {} : { owners: p.owners }),
    })
  }
  return out.sort((a, b) => a.name.localeCompare(b.name))
}

/** Who owns an offer, worth saying only where the offers have more than one owner between them. */
export function ownersLine(offers: readonly Offer[]): (o: Offer) => string | undefined {
  const all = new Set(offers.flatMap((o) => o.owners ?? []))
  return (o) => (all.size > 1 && o.owners !== undefined && o.owners.length > 0 ? o.owners.join(", ") : undefined)
}
