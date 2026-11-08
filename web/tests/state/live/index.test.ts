import { describe, expect, it } from "vitest"
import { createLive } from "@/state/live"
import { snapshot } from "../../kit"

describe("state/live", () => {
  it("starts connecting, goes live with a snapshot, keeps it while reconnecting", () => {
    const live = createLive()
    expect(live.store.get()).toEqual({ connection: "connecting", snapshot: undefined, recent: [], keep: 120 })
    const s = snapshot()
    live.snapshot(s)
    expect(live.store.get()).toEqual({ connection: "live", snapshot: s, recent: [s], keep: 120 })
    live.paused()
    expect(live.store.get()).toMatchObject({ connection: "reconnecting", snapshot: s })
    const before = live.store.get()
    live.paused()
    expect(live.store.get()).toBe(before)
  })
  it("keeps the latest snapshots, as many as asked", () => {
    const live = createLive({ keep: 2 })
    const [a, b, c] = [1, 2, 3].map((n) => snapshot({ now_ms: n }))
    for (const s of [a, b, c]) if (s) live.snapshot(s)
    expect(live.store.get().recent.map((s) => s.now_ms)).toEqual([2, 3])
  })
})
