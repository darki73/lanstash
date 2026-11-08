import { describe, expect, it } from "vitest"
import { runsReader } from "@/adapters/api/runs"
import type { Fetch } from "@/adapters/api/version"

const answering =
  (ok: boolean, body: unknown): Fetch =>
  async () => ({ ok, json: async () => body })

const run = { platform: "steam", job: { game: "7", state: "done", started_ms: 1, finished_ms: 9, bytes: 5 } }

describe("adapters/api/runs", () => {
  it("asks the API for a platform's prefill jobs that ended", async () => {
    const asked: string[] = []
    const fetch: Fetch = async (url) => {
      asked.push(url)
      return { ok: true, json: async () => ({ runs: [run] }) }
    }
    expect(await runsReader(fetch, "http://box")("epic games")).toEqual([run])
    expect(asked).toEqual(["http://box/api/v1/prefill/runs?platform=epic%20games&limit=50"])
  })
  it("leaves out what is not a run", async () => {
    const odd = [{ ...run, job: { ...run.job, state: "odd" } }, { platform: 1, job: run.job }, { platform: "x" }]
    expect(await runsReader(answering(true, { runs: [run, ...odd] }), "")("steam")).toEqual([run])
  })
  it("gives undefined for an error, a body that is not runs, or a failed request", async () => {
    expect(await runsReader(answering(false, { runs: [] }), "")("steam")).toBeUndefined()
    expect(await runsReader(answering(true, { runs: 1 }), "")("steam")).toBeUndefined()
    expect(
      await runsReader(async () => {
        throw new Error("offline")
      }, "")("steam"),
    ).toBeUndefined()
  })
})
