/**
 * Span: a length of time, measured in the unit that reads well (the words are the reader's
 * language's, from i18n): seconds under a minute and a half, minutes (one decimal under ten)
 * under an hour and a half, then hours to one decimal.
 */
export interface Span {
  readonly unit: "seconds" | "minutes" | "hours"
  readonly count: number
}

export function span(seconds: number): Span {
  const s = Math.max(0, Math.round(seconds))
  if (s < 90) return { unit: "seconds", count: s }
  const m = s / 60
  if (m < 90) return { unit: "minutes", count: Math.round(m) === 1 ? 1 : Number(m.toFixed(m < 10 ? 1 : 0)) }
  return { unit: "hours", count: Number((m / 60).toFixed(1)) }
}
