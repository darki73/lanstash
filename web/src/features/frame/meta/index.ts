/**
 * Meta: what the page's title says of lanstash at work: how fast it fetches from the internet
 * (downloads and prefills) and how fast it sends to the clients, each only while it is at least
 * 0.1 MB/s; one decimal under 10 MB/s, whole numbers above.
 */
import type { Snapshot } from "@/contracts/status"

/** Below this (MB/s) a rate is no activity worth a title. */
export const QUIET_MB_S = 0.1

export interface Activity {
  /** From the internet into the cache, MB/s as written. */
  readonly fetching?: string
  /** From the cache to the clients, MB/s as written. */
  readonly serving?: string
}

const written = (mbS: number) => (mbS < 10 ? mbS.toFixed(1) : String(Math.round(mbS)))

export function activity(snapshot: Snapshot | undefined): Activity {
  const rate = (name: string) => snapshot?.rates_mb_s[name] ?? 0
  const fetched = rate("downloads.fetched_bytes")
  const served = rate("lifetime.served_bytes")
  return {
    ...(fetched >= QUIET_MB_S ? { fetching: written(fetched) } : {}),
    ...(served >= QUIET_MB_S ? { serving: written(served) } : {}),
  }
}
