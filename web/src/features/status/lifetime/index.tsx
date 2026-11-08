/**
 * Lifetime: what lanstash did for the network, as the band at the top of the overview's home.
 * Over the overview's range it says the bytes clients got from the stash (each one the internet
 * did not have to send) against what they got while lanstash downloaded it, so the share is of
 * what clients asked for alone; what lanstash fetched beyond that (prefills, updates, pieces read
 * ahead) is a fact of its own, as are the bytes from the stash since lanstash first counted them.
 * Without counters kept over time it says the same since lanstash first counted (or started).
 */
import type { Snapshot } from "@/contracts/status"
import type { Kept } from "@/features/status/kept"
import { counter } from "@/features/status/model"
import { spanOf } from "@/features/status/range"
import { useT, type Words } from "@/i18n"
import { size } from "@/lib/bytes"
import { Ledger, type LedgerFact } from "@/styled/ledger"

const months = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"] as const

/** A day as "5 Nov 2026" (UTC), the same on every machine. */
export function day(ms: number, t: Words<"status">): string {
  const d = new Date(ms)
  const month = t(`date.month.${months[d.getUTCMonth()] ?? "jan"}`)
  return t("date.day", { day: d.getUTCDate(), month, year: d.getUTCFullYear() })
}

/** Bytes served from the stash and fetched from the internet, and since when (undefined: since this start). */
export function totals(s: Snapshot): { served: number; fetched: number; since: number | undefined } {
  const life = s.counters.lifetime
  if (life !== undefined) {
    return { served: life.served_bytes ?? 0, fetched: life.fetched_bytes ?? 0, since: life.since_ms }
  }
  return {
    served: counter(s, "requests", "hit_bytes"),
    fetched: counter(s, "downloads", "fetched_bytes") + counter(s, "pass-through", "bytes"),
    since: undefined,
  }
}

/** Over a range: what clients got from the stash, while downloading, and what was fetched besides. */
export function savings(k: Kept): { fromStash: number; downloading: number; besides: number } {
  const fromStash = k.total("requests.hit_bytes")
  const downloading = k.total("requests.miss_bytes")
  return { fromStash, downloading, besides: Math.max(0, k.total("downloads.fetched_bytes") - downloading) }
}

const share = (part: number, whole: number) => (whole > 0 ? part / whole : 0)

export function Lifetime({ snapshot, kept }: { readonly snapshot: Snapshot; readonly kept?: Kept | null }) {
  const t = useT("status")
  const life = totals(snapshot)
  const since = life.since === undefined ? undefined : day(life.since, t)
  if (kept) {
    const s = savings(kept)
    const asked = s.fromStash + s.downloading
    const facts: LedgerFact[] = [
      { label: t("lifetime.besides"), value: size(s.besides), tone: "origin" },
      {
        label:
          since === undefined ? t("lifetime.fromStashSinceStart") : t("lifetime.fromStashSinceDay", { day: since }),
        value: size(life.served),
        tone: "cache",
      },
    ]
    return (
      <Ledger
        label={t("lifetime.label")}
        headline={size(s.fromStash)}
        caption={t("lifetime.captionRange", { range: spanOf(t, kept.range) })}
        split={[
          {
            label: t("lifetime.fromStash"),
            value: size(s.fromStash),
            share: share(s.fromStash, asked),
            tone: "cache",
          },
          {
            label: t("lifetime.fromInternet"),
            value: size(s.downloading),
            share: share(s.downloading, asked),
            tone: "origin",
          },
        ]}
        rows={[]}
        facts={facts}
      />
    )
  }
  const all = life.served + life.fetched
  if (all === 0) return null
  return (
    <Ledger
      label={t("lifetime.label")}
      headline={size(life.served)}
      caption={since === undefined ? t("lifetime.captionSinceStart") : t("lifetime.captionSinceDay", { day: since })}
      split={[
        { label: t("lifetime.fromStash"), value: size(life.served), share: life.served / all, tone: "cache" },
        {
          label: t("lifetime.fromInternet"),
          value: size(life.fetched),
          share: life.fetched / all,
          tone: "origin",
        },
      ]}
      rows={[]}
    />
  )
}
