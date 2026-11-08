/**
 * Clamp: a fraction kept between 0 and 1 for drawing (a bar's width); what is not a number at all
 * (NaN, an infinity from a division by nothing) is 0, never a width the browser drops.
 */
export function fraction(x: number): number {
  return Number.isFinite(x) ? Math.min(1, Math.max(0, x)) : 0
}
