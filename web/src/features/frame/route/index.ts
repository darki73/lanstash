/**
 * Route: where in the app a path leads.
 *   /overview[/<resource>]  the overview: its home, or the page of a resource (throughput,
 *                           downloads, requests, storage, connections);
 *   /platforms/<id>         a platform (lanstash's service name: blizzard, steam, epicgames,
 *                           riot) and its cached games;
 *   /platforms/<id>/<part>  another part of a platform's page (library, prefill, account,
 *                           activity);
 *   /platforms/<id>/<game>  one game of a platform (its id there: a Battle.net CDN directory,
 *                           a Steam app id), cached or not.
 *   /settings               the settings the web UI may change;
 *   /dns                    the network's DNS server: its setup, its rewrites, what went to the internet;
 *   /history                what was updated when: new builds, games the stash came to hold whole;
 *   /health                 the checks lanstash makes of itself;
 *   /clients                who downloads what through the cache;
 *   /sign-in                signing in to lanstash;
 *   /account                one's own account;
 *   /accounts               every account, and the invites;
 *   /join/<token>           making an account with an invite (its token: 64 hex digits).
 * Anything else, an unknown resource included, leads to the overview. The links that lead to each
 * place are made by the feature that owns it.
 */
import { type Section, sections } from "@/features/platforms/model"
import { type ResourceId, resources } from "@/features/status/resource"

export type Place =
  | { readonly kind: "overview"; readonly resource: ResourceId | undefined }
  | { readonly kind: "settings" }
  | { readonly kind: "dns" }
  | { readonly kind: "history" }
  | { readonly kind: "health" }
  | { readonly kind: "clients" }
  | { readonly kind: "sign-in" }
  | { readonly kind: "join"; readonly token: string }
  | { readonly kind: "account" }
  | { readonly kind: "accounts" }
  | { readonly kind: "platform"; readonly platform: string; readonly section: Section }
  | { readonly kind: "game"; readonly platform: string; readonly game: string }

const home: Place = { kind: "overview", resource: undefined }

/** The sidebar's link to the overview. */
export const overviewHome = "/overview"

/** The sidebar's link to the settings. */
export const settingsHome = "/settings"

/** The sidebar's link to the DNS page. */
export const dnsHome = "/dns"

/** The sidebar's link to the history. */
export const historyHome = "/history"

/** The sidebar's link to the health page. */
export const healthHome = "/health"

/** The sidebar's link to the clients page. */
export const clientsHome = "/clients"

const isResource = (s: string): s is ResourceId => (resources as readonly string[]).includes(s)
const isSection = (s: string): s is Section => (sections as readonly string[]).includes(s)

export function place(path: string): Place {
  const [, section, name, game, ...rest] = path.split("/")
  if (section === "overview" && name !== undefined && isResource(name)) return { kind: "overview", resource: name }
  if (section === "settings" && (name === undefined || name === "")) return { kind: "settings" }
  if (section === "dns" && (name === undefined || name === "")) return { kind: "dns" }
  if (section === "history" && (name === undefined || name === "")) return { kind: "history" }
  if (section === "health" && (name === undefined || name === "")) return { kind: "health" }
  if (section === "clients" && (name === undefined || name === "")) return { kind: "clients" }
  if (section === "sign-in" && (name === undefined || name === "")) return { kind: "sign-in" }
  if (section === "account" && (name === undefined || name === "")) return { kind: "account" }
  if (section === "accounts" && (name === undefined || name === "")) return { kind: "accounts" }
  if (section === "join" && name !== undefined && /^[0-9a-f]{64}$/.test(name) && game === undefined)
    return { kind: "join", token: name }
  if (section === "platforms" && name !== undefined && /^[a-z0-9-]+$/.test(name) && rest.length === 0) {
    if (game !== undefined && game !== "cached" && isSection(game))
      return { kind: "platform", platform: name, section: game }
    if (game !== undefined && /^[A-Za-z0-9_-]{1,64}$/.test(game)) return { kind: "game", platform: name, game }
    if (game === undefined || game === "") return { kind: "platform", platform: name, section: "cached" }
  }
  return home
}
