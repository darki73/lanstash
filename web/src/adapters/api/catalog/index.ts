/**
 * Catalog: asks lanstash's API for a platform's games (GET /api/v1/catalog/<platform>: every
 * Battle.net game there is, the Steam account's games with their download size).
 * An answer that is not a catalog (an error, a lanstash without the route, no catalog yet) is
 * undefined, never an exception. The fetch is passed in.
 */
import type { Fetch } from "@/adapters/api/version"

export interface CatalogAnswer {
  readonly products: readonly {
    readonly code: string
    readonly name: string | null
    readonly game: string | null
    readonly version: string | null
    readonly known: boolean
    readonly download?: number
    readonly lancache?: { readonly have: number; readonly want: number }
    /** The accounts that own it (Steam). */
    readonly owners?: readonly string[]
    /** The operating systems it installs for, when more than one (Steam). */
    readonly systems?: readonly string[]
  }[]
}

const isNames = (o: unknown) => o === undefined || (Array.isArray(o) && o.every((n) => typeof n === "string"))

const isProduct = (p: unknown) =>
  typeof p === "object" &&
  p !== null &&
  typeof (p as { code?: unknown }).code === "string" &&
  typeof (p as { known?: unknown }).known === "boolean" &&
  isNames((p as { owners?: unknown }).owners) &&
  isNames((p as { systems?: unknown }).systems)

const isAnswer = (v: unknown): v is CatalogAnswer =>
  typeof v === "object" &&
  v !== null &&
  Array.isArray((v as { products?: unknown }).products) &&
  (v as { products: unknown[] }).products.every(isProduct)

export async function readCatalog(
  fetch: Fetch,
  apiBase: string,
  platform = "blizzard",
): Promise<CatalogAnswer | undefined> {
  try {
    const answer = await fetch(`${apiBase}/api/v1/catalog/${encodeURIComponent(platform)}`)
    if (!answer.ok) return undefined
    const body = await answer.json()
    return isAnswer(body) ? body : undefined
  } catch {
    return undefined
  }
}
