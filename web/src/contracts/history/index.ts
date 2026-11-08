/**
 * History: what was updated when (GET /api/v1/history), newest first. lanstash notes, each time it
 * works out a platform's builds, what changed since it last did: a game seen for the first time
 * ("added"), a new build of a game ("new-build", with the build before), and the stash coming to
 * hold all of a game's build ("current").
 */

export type HistoryChange = "added" | "new-build" | "current"

export interface HistoryEvent {
  readonly at_ms: number
  /** lanstash's service name: blizzard, steam, epicgames. */
  readonly platform: string
  /** The game's id there (a Battle.net product code, a Steam app id, an Epic app). */
  readonly game: string
  readonly change: HistoryChange
  /** The build it is about. */
  readonly version: string
  /** The build before, for a new build. */
  readonly from: string | null
  /** lanstash masked the event (a hidden game, shown to someone who is not an admin). */
  readonly hidden?: boolean
}

export interface History {
  readonly events: readonly HistoryEvent[]
}

/** The history before lanstash said it (or from a lanstash that keeps none). */
export const noHistory: History = { events: [] }

/** The events of one game, newest first. */
export const eventsOf = (history: History, platform: string, game: string): readonly HistoryEvent[] =>
  history.events.filter((e) => e.platform === platform && e.game === game)
