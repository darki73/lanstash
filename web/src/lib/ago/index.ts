/**
 * Ago: how long ago something happened, measured in the unit that reads at a glance (the words are
 * the reader's language's, from i18n): never (time 0), just now (under a minute, or a time in the
 * future: clocks of two machines), minutes under an hour, hours under two days, days under two
 * weeks, then weeks.
 */
export type Since =
  | { readonly unit: "never" | "now" }
  | { readonly unit: "minutes" | "hours" | "days" | "weeks"; readonly count: number }

export function since(nowMs: number, thenMs: number): Since {
  if (thenMs <= 0) return { unit: "never" }
  const s = Math.max(0, (nowMs - thenMs) / 1000)
  if (s < 60) return { unit: "now" }
  const m = Math.floor(s / 60)
  if (m < 60) return { unit: "minutes", count: m }
  const h = Math.floor(m / 60)
  if (h < 48) return { unit: "hours", count: h }
  const d = Math.floor(h / 24)
  if (d < 14) return { unit: "days", count: d }
  return { unit: "weeks", count: Math.floor(d / 7) }
}
