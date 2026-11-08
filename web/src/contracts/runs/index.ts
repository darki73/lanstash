/**
 * Runs: the prefill jobs that ended, as lanstash kept them beyond a restart (GET
 * /api/v1/prefill/runs), newest first, each with its platform. A RunsReader asks for a platform's;
 * noRuns is the reader of a page that has none. took() says how long a run took.
 */
import type { Prefill } from "@/contracts/prefill"

export interface PrefillRun {
  readonly platform: string
  readonly job: Prefill
}

/** A platform's last runs, newest first; undefined when lanstash keeps none or could not answer. */
export type RunsReader = (platform: string) => Promise<readonly PrefillRun[] | undefined>

export const noRuns: RunsReader = async () => undefined

/** How long a run took (ms), once it ended. */
export const took = (job: Prefill): number | undefined =>
  job.finished_ms === null ? undefined : Math.max(0, job.finished_ms - job.started_ms)
