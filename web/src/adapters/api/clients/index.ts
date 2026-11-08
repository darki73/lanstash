/**
 * Clients: asks lanstash's API what each client got over a range (GET
 * /api/v1/clients?from=&to=) and names clients (PUT /api/v1/clients/<address>?label=<name>, the
 * name escaped; DELETE /api/v1/clients/<address> takes it away). An answer that is not a list of
 * clients (an error, an older lanstash without the route) reads as undefined, never an exception;
 * a refused name gives the API's reason. The requests are passed in.
 */
import type { Fetch } from "@/adapters/api/version"

/** One client's totals as the API answers them. */
export interface ApiClientTotals {
  readonly address: string
  readonly label: string | null
  readonly host: string | null
  readonly hit: number
  readonly miss: number
  readonly games: readonly {
    readonly service: string
    readonly game: string
    readonly hit: number
    readonly miss: number
  }[]
}

const text = (v: unknown) => typeof v === "string"
const textOrNull = (v: unknown) => v === null || typeof v === "string"
const count = (v: unknown) => typeof v === "number" && Number.isFinite(v)

const isGame = (v: unknown) => {
  if (typeof v !== "object" || v === null) return false
  const g = v as Record<string, unknown>
  return text(g.service) && text(g.game) && count(g.hit) && count(g.miss)
}

const isClient = (v: unknown): v is ApiClientTotals => {
  if (typeof v !== "object" || v === null) return false
  const c = v as Record<string, unknown>
  return (
    text(c.address) &&
    textOrNull(c.label) &&
    textOrNull(c.host) &&
    count(c.hit) &&
    count(c.miss) &&
    Array.isArray(c.games) &&
    c.games.every(isGame)
  )
}

export const clientsReader =
  (fetch: Fetch, apiBase: string) =>
  async (from_ms: number, to_ms: number): Promise<readonly ApiClientTotals[] | undefined> => {
    try {
      const answer = await fetch(`${apiBase}/api/v1/clients?from=${Math.floor(from_ms)}&to=${Math.floor(to_ms)}`)
      if (!answer.ok) return undefined
      const list = ((await answer.json()) as { clients?: unknown } | null)?.clients
      return Array.isArray(list) && list.every(isClient) ? list : undefined
    } catch {
      return undefined
    }
  }

const reason = (body: unknown) =>
  typeof body === "object" && body !== null && typeof (body as { error?: unknown }).error === "string"
    ? (body as { error: string }).error
    : "lanstash refused"

/** Names a client (PUT), or takes its name away (DELETE) when the name is empty. */
export const clientNamer =
  (put: Fetch, del: Fetch, apiBase: string) =>
  async (address: string, label: string): Promise<string | undefined> => {
    const at = `${apiBase}/api/v1/clients/${encodeURIComponent(address)}`
    const name = label.trim()
    try {
      const answer = await (name === "" ? del(at) : put(`${at}?label=${encodeURIComponent(name)}`))
      return answer.ok ? undefined : reason(await answer.json().catch(() => null))
    } catch {
      return "lanstash did not answer"
    }
  }
