/**
 * Version: what lanstash's API answers on /api/v1/version: the version running, and the newest
 * release when lanstash has looked one up (not yet: the field is absent until it does).
 */
export interface Version {
  readonly version: string
  /** The newest release, when known. */
  readonly latest?: string | null
}

/** updateAvailable is the newer release to offer, if there is one. */
export function updateAvailable(v: Version): string | undefined {
  return v.latest !== undefined && v.latest !== null && v.latest !== v.version ? v.latest : undefined
}
