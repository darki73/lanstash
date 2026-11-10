/**
 * Version: what lanstash's API answers on /api/v1/version: the version running, and the newest
 * release when it is newer than that (lanstash asks GitHub every few hours, unless check_updates
 * is off; the field is absent until it knows of one).
 */
export interface Version {
  readonly version: string
  /** The newest release, when it is newer than the version running. */
  readonly latest?: string | null
}

/** updateAvailable is the newer release to offer, if there is one. */
export function updateAvailable(v: Version): string | undefined {
  return v.latest !== undefined && v.latest !== null && v.latest !== v.version ? v.latest : undefined
}
