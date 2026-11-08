/**
 * Art: the API's address of a game's image, GET /api/v1/art/<platform>/<id> for its cover and
 * .../<kind> for its icon, background or logo, the parts escaped. The browser loads it as an image; lanstash answers 404
 * when it has none.
 */

export const artLink =
  (apiBase: string) =>
  (platform: string, id: string, kind?: "header" | "icon" | "background" | "logo"): string =>
    `${apiBase}/api/v1/art/${encodeURIComponent(platform)}/${encodeURIComponent(id)}${kind === undefined || kind === "header" ? "" : `/${kind}`}`
