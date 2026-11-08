/**
 * Private: the games a signed-in player keeps to themselves (GET /api/v1/private), each as
 * `<platform>/<game>`. Only that player's having the game is kept private: anyone else sees it as
 * a hidden game when every owner marked it, and plainly when another owner did not; admins see
 * all. PrivateActions reads the caller's list and marks or unmarks one of its own games;
 * noPrivate is the page that cannot.
 */

export interface PrivateActions {
  /** The caller's private games; undefined when lanstash could not say. */
  readonly read: () => Promise<readonly string[] | undefined>
  /** Keeps a game private, or takes the mark away: the list as it is after, or why not. */
  readonly set: (platform: string, game: string, on: boolean) => Promise<readonly string[] | string>
}

export const noPrivate: PrivateActions = {
  read: async () => undefined,
  set: async () => "lanstash keeps no private games",
}
