/**
 * Runs: asks lanstash's API for a platform's prefill jobs that ended (GET
 * /api/v1/prefill/runs?platform=<id>), newest first. An answer that is not a list of runs (an
 * error, an older lanstash without the route) is undefined, never an exception; a run without a
 * game, a state, times or bytes is left out. The fetch is passed in.
 */
import type { Fetch } from "@/adapters/api/version"

export interface RunAnswer {
  readonly platform: string
  readonly job: {
    readonly game: string
    readonly state: "running" | "done" | "stopped" | "failed"
    readonly started_ms: number
    readonly finished_ms: number | null
    readonly bytes: number
    readonly [field: string]: unknown
  }
}

const states: readonly string[] = ["running", "done", "stopped", "failed"]

const isRun = (r: unknown): r is RunAnswer => {
  if (typeof r !== "object" || r === null) return false
  const { platform, job } = r as { platform?: unknown; job?: Record<string, unknown> | null }
  return (
    typeof platform === "string" &&
    typeof job === "object" &&
    job !== null &&
    typeof job.game === "string" &&
    typeof job.state === "string" &&
    states.includes(job.state) &&
    typeof job.started_ms === "number" &&
    typeof job.bytes === "number" &&
    (job.finished_ms === null || typeof job.finished_ms === "number")
  )
}

export const runsReader =
  (fetch: Fetch, apiBase: string) =>
  async (platform: string): Promise<RunAnswer[] | undefined> => {
    try {
      const answer = await fetch(`${apiBase}/api/v1/prefill/runs?platform=${encodeURIComponent(platform)}&limit=50`)
      if (!answer.ok) return undefined
      const body: unknown = await answer.json()
      const runs = (body as { runs?: unknown } | null)?.runs
      return Array.isArray(runs) ? runs.filter(isRun) : undefined
    } catch {
      return undefined
    }
  }
