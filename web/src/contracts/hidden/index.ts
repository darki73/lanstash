/**
 * Hidden: the games kept private (GET /api/v1/hidden), each as `<platform>/<game>` (a Steam app
 * id, a Battle.net product code). A hidden game is still cached, updated and prefilled; the page
 * masks it wherever it would show (its name, art, page and builds), unless this browser is set to
 * show hidden games. HiddenActions hides and shows one; noHiding is the page that cannot.
 */

export type Hidden = readonly string[]

/** What a masked game is called. */
export const HIDDEN_NAME = "A hidden game"

/** A game's key in the list. */
export const hiddenKey = (platform: string, game: string) => `${platform}/${game}`

/** How a game shows: masked (it is hidden), revealed (hidden, shown to this browser), or plainly. */
export type Privacy = "masked" | "revealed" | undefined

export interface HiddenActions {
  /** Hides a game, or shows it again: the list as it is after, or why not. */
  readonly set: (platform: string, game: string, hidden: boolean) => Promise<Hidden | string>
}

export const noHiding: HiddenActions = { set: async () => "lanstash keeps no hidden games" }

/** How a game shows, given the hidden games and whether this browser shows them. */
export function privacyOf(hidden: Hidden | null | undefined, reveal: boolean, platform: string, game: string): Privacy {
  if (!hidden?.includes(hiddenKey(platform, game))) return undefined
  return reveal ? "revealed" : "masked"
}
