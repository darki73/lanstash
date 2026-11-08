/**
 * Art: where a game's images are. The API serves them (GET /api/v1/art/<platform>/<id>[/<kind>]),
 * fetched once from the platform's CDN and kept by lanstash; the id is a Steam app id, a
 * Battle.net product path, or a Battle.net install's product code (its own images). The header is
 * a cover (Steam's wide banner, Battle.net's portrait key art); the icon is square; the background
 * is the large wide image behind a game's page (Steam's library hero, Battle.net's launcher
 * background); the logo is the game's logo on a clear ground. An ArtLink turns a platform, an id and a kind into the image's
 * address; noArt is the link for a page that shows no art.
 */

export type ArtKind = "header" | "icon" | "background" | "logo"

/** The address of an image of game id of a platform, or undefined for none. */
export type ArtLink = (platform: string, id: string, kind?: ArtKind) => string | undefined

export const noArt: ArtLink = () => undefined
