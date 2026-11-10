import { describe, expect, it, vi } from "vitest"
import type { Fetch } from "@/adapters/api/version"
import { buildsFallback, compose, qrReads, signInEvery, statusEvents, versionEvery, versionSoon } from "@/app/compose"
import { harness, snapshot } from "../../kit"

const quietWindow = {
  location: { pathname: "/platforms/steam", hash: "" },
  history: { pushState() {}, replaceState() {} },
  addEventListener() {},
  removeEventListener() {},
}
const version: Fetch = async () => ({ ok: true, json: async () => ({ version: "0.1.0" }) })

/** The session of a lanstash without accounts: everyone may do everything. */
const everything = { name: null, role: "admin", accounts: false }

describe("app/compose", () => {
  it("opens nothing until started, then feeds the status from the stream until stopped", async () => {
    const { sources, open, later } = harness()
    const c = compose({ apiBase: "", openSource: open, fetch: version, window: quietWindow, later })
    expect(sources).toHaveLength(0)
    expect(c.version.get()).toBeUndefined()
    const stop = c.start()
    expect(sources[0]?.url).toBe("/api/v1/events")
    const s = snapshot()
    sources[0]?.emit("message", JSON.stringify(s))
    expect(c.status.get()).toMatchObject({ connection: "live", snapshot: s, recent: [s] })
    await vi.waitFor(() => expect(c.version.get()).toEqual({ version: "0.1.0" }))
    stop()
    expect(sources[0]?.closed).toBe(true)
  })
  it("asks for the version again soon, then now and then, so a newer release shows on an open page", async () => {
    const { open } = harness()
    const put: { run: () => void; ms: number }[] = []
    const later = (run: () => void, ms: number) => put.push({ run, ms })
    let asked = 0
    const fetch: Fetch = async (url) => {
      const body = url.endsWith("/api/v1/version")
        ? { version: "0.1.0", ...(++asked > 1 ? { latest: "0.2.0" } : {}) }
        : {}
      return { ok: true, json: async () => body }
    }
    const c = compose({ apiBase: "", openSource: open, fetch, window: quietWindow, later })
    const stop = c.start()
    await vi.waitFor(() => expect(c.version.get()).toEqual({ version: "0.1.0" }))
    const next = () => {
      const at = put.findIndex((p) => p.ms === versionSoon || p.ms === versionEvery)
      const [p] = put.splice(at, 1)
      return p
    }
    await vi.waitFor(() => expect(put.some((p) => p.ms === versionSoon)).toBe(true))
    next()?.run()
    await vi.waitFor(() => expect(c.version.get()).toEqual({ version: "0.1.0", latest: "0.2.0" }))
    await vi.waitFor(() => expect(put.some((p) => p.ms === versionEvery)).toBe(true))
    stop()
    const before = asked
    next()?.run()
    await new Promise((r) => setTimeout(r, 10))
    expect(asked, "stopped: not asked again").toBe(before)
  })
  it("reads the app's path from the window", () => {
    const { open } = harness()
    const c = compose({ apiBase: "", openSource: open, fetch: version, window: quietWindow })
    expect(c.path.get()).toBe("/platforms/steam")
  })
  it("points games' art at the API", () => {
    const { open } = harness()
    const c = compose({ apiBase: "http://box", openSource: open, fetch: version, window: quietWindow })
    expect(c.art("steam", "7")).toBe("http://box/api/v1/art/steam/7")
  })
  it("asks for the Battle.net builds on start, then again each time the timer fires", async () => {
    const { open, pending, later } = harness()
    let asked = 0
    const fetch: Fetch = async (url) => {
      if (url.endsWith("/builds/blizzard")) asked += 1
      const body = url.endsWith("/builds/blizzard") ? { region: "eu", games: [] } : { version: "0.1.0" }
      return { ok: true, json: async () => body }
    }
    const c = compose({ apiBase: "", openSource: open, fetch, window: quietWindow, later })
    const stop = c.start()
    await vi.waitFor(() => expect(c.builds.get().blizzard).toEqual({ region: "eu", games: [] }))
    expect(c.builds.get().steam, "the fake answers only Battle.net's").toBeUndefined()
    expect(asked).toBe(1)
    await vi.waitFor(() => expect(pending.length).toBeGreaterThan(0))
    for (const run of pending.splice(0)) run()
    await vi.waitFor(() => expect(asked).toBe(2))
    stop()
    for (const run of pending.splice(0)) run()
    expect(asked).toBe(2)
  })
  it("shows a platform's builds as they arrive, while a slow platform's are still on the way", async () => {
    const { open, later } = harness()
    const fetch: Fetch = (url) =>
      url.endsWith("/builds/blizzard")
        ? new Promise(() => {})
        : Promise.resolve({
            ok: true,
            json: async () => (url.endsWith("/builds/epicgames") ? { region: "windows", games: [] } : {}),
          })
    const c = compose({ apiBase: "", openSource: open, fetch, window: quietWindow, later })
    const stop = c.start()
    await vi.waitFor(() => expect(c.builds.get().epicgames).toEqual({ region: "windows", games: [] }))
    expect(c.builds.get().blizzard).toBeUndefined()
    stop()
  })
  it("asks for the builds again as soon as a prefill ends", async () => {
    const { sources, open, later } = harness()
    let asked = 0
    const fetch: Fetch = async (url) => {
      if (url.endsWith("/builds/blizzard")) asked += 1
      return { ok: true, json: async () => ({ region: "eu", games: [] }) }
    }
    const c = compose({ apiBase: "", openSource: open, fetch, window: quietWindow, later })
    const stop = c.start()
    await vi.waitFor(() => expect(asked).toBe(1))
    const job = {
      game: "hs",
      version: "1",
      state: "running" as const,
      files_total: 2,
      files_done: 1,
      files_skipped: 0,
      files_missing: 0,
      bytes: 1,
      current: null,
      error: null,
      started_ms: 0,
      finished_ms: null,
    }
    const emit = (prefill: typeof job | { state: "done"; finished_ms: number }) =>
      sources[0]?.emit("message", JSON.stringify(snapshot({ prefill: { ...job, ...prefill } })))
    emit(job)
    emit(job)
    expect(asked).toBe(1)
    emit({ state: "done", finished_ms: 99 })
    await vi.waitFor(() => expect(asked).toBe(2))
    emit({ state: "done", finished_ms: 99 })
    await new Promise((r) => setTimeout(r, 10))
    expect(asked).toBe(2)
    stop()
  })
  it("asks for a platform's builds again when the stream says they changed, and only that platform's", async () => {
    const { sources, open } = harness()
    const waits: { run: () => void; ms: number }[] = []
    const later = (run: () => void, ms: number) => waits.push({ run, ms })
    const asked: Record<string, number> = {}
    let histories = 0
    const event = { at_ms: 1, platform: "steam", game: "7", change: "current", version: "9", from: null }
    const fetch: Fetch = async (url) => {
      const p = url.split("/builds/")[1]
      if (p !== undefined) asked[p] = (asked[p] ?? 0) + 1
      if (url.endsWith("/history")) {
        histories += 1
        return { ok: true, json: async () => ({ events: [event] }) }
      }
      return { ok: true, json: async () => ({ region: "eu", games: [] }) }
    }
    const c = compose({ apiBase: "", openSource: open, fetch, window: quietWindow, later })
    const stop = c.start()
    await vi.waitFor(() => expect(asked.epicgames).toBe(1))
    const emit = (marks: Record<string, number>) =>
      sources[0]?.emit("message", JSON.stringify(snapshot({ builds_changed: marks })))
    emit({ blizzard: 5, steam: 5, epicgames: 5 })
    emit({ blizzard: 5, steam: 5, epicgames: 5 })
    await new Promise((r) => setTimeout(r, 10))
    expect(asked).toEqual({ blizzard: 1, steam: 1, epicgames: 1 })
    expect(histories, "the history is read with the builds").toBe(1)
    expect(c.history.get().events).toEqual([event])
    emit({ blizzard: 5, steam: 9, epicgames: 5 })
    await vi.waitFor(() => expect(asked.steam).toBe(2))
    expect(asked).toEqual({ blizzard: 1, steam: 2, epicgames: 1 })
    await vi.waitFor(() => expect(histories, "and again when builds change").toBe(2))
    // Told, asking every so often is only a fallback: the next round waits five minutes.
    await vi.waitFor(() => expect(waits.some((w) => w.ms === buildsFallback)).toBe(true))
    stop()
  })
  it("starts prefills through the POST it was given, and has none without one", async () => {
    const { open } = harness()
    const posted: string[] = []
    const post = async (url: string) => {
      posted.push(url)
      return { ok: true, json: async () => ({ started: "hs" }) }
    }
    const c = compose({ apiBase: "", openSource: open, fetch: version, post, window: quietWindow })
    expect(await c.prefill.start("blizzard", "hs")).toBeUndefined()
    expect(posted).toEqual(["/api/v1/prefill/blizzard/hs"])
    const without = compose({ apiBase: "", openSource: open, fetch: version, window: quietWindow })
    expect(await without.prefill.start("blizzard", "hs")).toBe("prefill is not available here")
  })
  it("asks a guest's page for no platform account nor hidden game", async () => {
    const { open } = harness()
    const asked: string[] = []
    const fetch: Fetch = async (url) => {
      asked.push(url)
      if (url.endsWith("/api/v1/session")) {
        return { ok: true, json: async () => ({ name: null, role: "guest", accounts: true }) }
      }
      return { ok: true, json: async () => ({ version: "0.1.0" }) }
    }
    const ok = async () => ({ ok: true, json: async () => ({}) })
    const c = compose({ apiBase: "", openSource: open, fetch, post: ok, del: ok, put: ok, window: quietWindow })
    const stop = c.start()
    await vi.waitFor(() => expect(c.session.get().role).toBe("guest"))
    await new Promise((r) => setTimeout(r, 20))
    expect(asked.filter((u) => u.includes("/account/") || u.includes("/hidden"))).toEqual([])
    expect(c.accounts.get()).toEqual({})
    stop()
  })
  it("fetches a waiting sign-in's QR code again when it changed, or every few reads, not at every read", async () => {
    const { open } = harness()
    // The sign-in's next read, told from the other timers by its interval.
    let next: (() => void) | undefined
    const later = (run: () => void, ms: number) => {
      if (ms === signInEvery) next = run
    }
    let scanned = false
    const fetch: Fetch = async (url) => {
      if (url.endsWith("/api/v1/session")) return { ok: true, json: async () => everything }
      if (url.endsWith("/account/steam")) {
        const body = { accounts: [], sign_in: { state: "waiting", scanned, error: null } }
        return { ok: true, json: async () => body }
      }
      if (url.includes("/account/")) return { ok: false, json: async () => ({ error: "none", kind: "not-found" }) }
      return { ok: true, json: async () => ({ version: "0.1.0" }) }
    }
    const ok = async () => ({ ok: true, json: async () => ({}) })
    const c = compose({ apiBase: "", openSource: open, fetch, post: ok, del: ok, window: quietWindow, later })
    const stop = c.start()
    const turn = () => c.accounts.get().steam?.qr.split("turn=")[1]
    await vi.waitFor(() => expect(turn()).toBe("1"))
    await vi.waitFor(() => expect(next).toBeDefined())
    const again = async () => {
      const run = next
      next = undefined
      run?.()
      // Its answer is drawn once it asked for the read after it.
      await vi.waitFor(() => expect(next).toBeDefined())
    }
    for (let i = 1; i < qrReads; i++) {
      await again()
      expect(turn(), `read ${i + 1}`).toBe("1")
    }
    await again()
    expect(turn(), "as often as Steam turns its code").toBe("2")
    scanned = true
    await again()
    expect(turn(), "the sign-in changed").toBe("3")
    stop()
  })
  it("follows each platform's accounts, often while a sign-in is under way, and at once after an action", async () => {
    const { open, pending, later } = harness()
    let state = "idle"
    const reads: string[] = []
    const fetch: Fetch = async (url) => {
      if (url.endsWith("/api/v1/session")) return { ok: true, json: async () => everything }
      if (url.includes("/account/")) reads.push(url)
      if (url.endsWith("/account/epicgames")) {
        return { ok: false, json: async () => ({ error: "no account for this platform", kind: "not-found" }) }
      }
      const body = url.endsWith("/account/steam")
        ? { accounts: [], sign_in: { state, scanned: false, error: null } }
        : { version: "0.1.0" }
      return { ok: true, json: async () => body }
    }
    const posted: string[] = []
    const post = async (url: string) => {
      posted.push(url)
      state = "waiting"
      return { ok: true, json: async () => ({}) }
    }
    const del = async () => ({ ok: true, json: async () => ({}) })
    const c = compose({ apiBase: "", openSource: open, fetch, post, del, window: quietWindow, later })
    const stop = c.start()
    const steam = () => c.accounts.get().steam
    await vi.waitFor(() => expect(steam()?.view.sign_in.state).toBe("idle"))
    expect(steam()?.qr).toBe("/api/v1/account/steam/qr.svg?turn=1")
    await vi.waitFor(() => expect(c.accounts.get().epicgames, "lanstash offers none").toBeNull())
    expect(await c.accountActions("steam").signIn()).toBeUndefined()
    expect(await c.accountActions("epicgames").signInWithCode(" f00d ")).toBeUndefined()
    expect(posted).toEqual(["/api/v1/account/steam/sign-in", "/api/v1/account/epicgames/sign-in"])
    await vi.waitFor(() => expect(steam()?.view.sign_in.state).toBe("waiting"))
    expect(steam()?.qr, "a new address for the code").toBe("/api/v1/account/steam/qr.svg?turn=2")
    stop()
    const before = reads.length
    for (const run of pending.splice(0)) run()
    expect(reads.length).toBe(before)
  })
  it("has no account actions without a POST and a DELETE", async () => {
    const { open } = harness()
    const c = compose({ apiBase: "", openSource: open, fetch: version, window: quietWindow })
    expect(await c.accountActions("steam").signIn()).toBe("signing in is not available here")
  })
  it("puts the API base in front of the stream", () => {
    expect(statusEvents("http://192.0.2.10:8081")).toBe("http://192.0.2.10:8081/api/v1/events")
  })
})
