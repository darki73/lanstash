/**
 * Percent: how much of a whole is done, in whole percent rounded down, so a part never reads
 * "100 %" before it is all there ("< 1 %" for a start, "–" without a whole).
 */
export function done(part: number, whole: number): string {
  if (!(whole > 0) || !Number.isFinite(whole) || !Number.isFinite(part)) return "–"
  const share = Math.min(1, Math.max(0, part / whole))
  if (share > 0 && share < 0.01) return "< 1 %"
  return `${Math.floor(share * 100)} %`
}
