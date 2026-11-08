/**
 * Setup: the file that sets up a PC for a platform, where the platform's client needs that before
 * lanstash keeps its downloads (Epic Games on Windows). The API serves it to save
 * (GET /api/v1/setup/<platform>/<os>). A SetupLink turns a platform and an operating system into
 * the file's address; noSetup is the link for a page that offers no file.
 */

export type ClientOs = "windows" | "macos" | "linux"

/** The address of the file that sets up a PC running os for platform, or undefined for none. */
export type SetupLink = (platform: string, os: ClientOs) => string | undefined

export const noSetup: SetupLink = () => undefined
