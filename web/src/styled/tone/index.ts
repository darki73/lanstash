/**
 * Tone: the meaning a trace, a swatch or a tile's mark is drawn in. cache (amber) is bytes from
 * the stash, origin (cyan) is bytes from the internet, plain is everything else (ink). The class
 * names are written out whole so Tailwind finds them.
 */
export type Tone = "cache" | "origin" | "plain"

/** The text colour of a tone; drawings use it through currentColor. */
export const toneText: Readonly<Record<Tone, string>> = {
  cache: "text-cache",
  origin: "text-origin",
  plain: "text-muted",
}

export const toneBg: Readonly<Record<Tone, string>> = {
  cache: "bg-cache",
  origin: "bg-origin",
  plain: "bg-muted",
}
