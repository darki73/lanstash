/**
 * Count: a whole number with thousands separators ("12,345"), the same on every machine.
 */
export function count(n: number): string {
  return n.toLocaleString("en-US")
}
