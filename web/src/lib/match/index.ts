/**
 * Match: how well a name answers what was typed, for finding a game as one types. Case, accents
 * and punctuation do not count ("pokemon" finds "Pokémon"). Best is a name that starts with what was typed (4), then one with a word
 * that does (3), then one that holds it anywhere (2), then one whose words' initials start with it
 * ("gta" finds "Grand Theft Auto V", 1); a name that does not answer it at all has no score.
 * Nothing typed matches every name alike (0).
 */

/** A text as matching sees it: lower case, without accents, words split by single spaces. */
export function fold(text: string): string {
  return text
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim()
}

export function match(name: string, typed: string): number | undefined {
  const q = fold(typed)
  if (q === "") return 0
  const n = fold(name)
  if (n.startsWith(q)) return 4
  if (` ${n}`.includes(` ${q}`)) return 3
  if (n.includes(q)) return 2
  const initials = n
    .split(" ")
    .map((w) => w[0] ?? "")
    .join("")
  return initials.startsWith(q.replaceAll(" ", "")) ? 1 : undefined
}
