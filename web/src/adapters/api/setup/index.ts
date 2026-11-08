/**
 * Setup: the API's address of the file that sets up a PC for a platform,
 * GET /api/v1/setup/<platform>/<os>, the platform escaped. The browser saves it; lanstash answers
 * 404 where there is none.
 */

export const setupLink =
  (apiBase: string) =>
  (platform: string, os: "windows" | "macos" | "linux"): string =>
    `${apiBase}/api/v1/setup/${encodeURIComponent(platform)}/${os}`
