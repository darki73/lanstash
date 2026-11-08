import { describe, expect, it } from "vitest"
import { follow } from "@/adapters/api/live"
import { harness } from "../../../kit"

describe("adapters/api/live", () => {
  it("hands each message on as JSON and skips one that is not", () => {
    const { sources, open } = harness()
    const seen: unknown[] = []
    follow<{ n: number }>(open, "/api/v1/events", { event: (e) => seen.push(e) })
    const s = sources[0]
    s?.emit("message", '{"n":1}')
    s?.emit("message", "not json")
    s?.emit("message", '{"n":2}')
    expect(seen).toEqual([{ n: 1 }, { n: 2 }])
    expect(s?.url).toBe("/api/v1/events")
  })
  it("skips a message the follower does not accept", () => {
    const { sources, open } = harness()
    const seen: unknown[] = []
    const accept = (v: unknown): v is { n: number } => typeof (v as { n?: unknown } | null)?.n === "number"
    follow<{ n: number }>(open, "/api/v1/events", { event: (e) => seen.push(e), accept })
    for (const m of ["null", '{"n":"x"}', '{"n":3}']) sources[0]?.emit("message", m)
    expect(seen).toEqual([{ n: 3 }])
  })
  it("says paused when the connection drops, and lets the browser open it again", () => {
    const { sources, open, pending, later } = harness()
    let paused = 0
    follow(open, "/e", { event: () => {}, paused: () => paused++ }, { later })
    sources[0]?.emit("error")
    expect([paused, sources.length, pending.length]).toEqual([1, 1, 0])
  })
  it("opens a connection the browser gave up on again, after a pause", () => {
    const { sources, open, pending, later } = harness()
    follow(open, "/e", { event: () => {} }, { later })
    const first = sources[0]
    if (first) first.readyState = 2
    first?.emit("error")
    expect(sources.length).toBe(1)
    pending.shift()?.()
    expect(sources.length).toBe(2)
  })
  it("closes the connection when stopped, and opens nothing after", () => {
    const { sources, open, pending, later } = harness()
    const stop = follow(open, "/e", { event: () => {} }, { later })
    const first = sources[0]
    if (first) first.readyState = 2
    first?.emit("error")
    stop()
    pending.shift()?.()
    expect([first?.closed, sources.length]).toEqual([true, 1])
  })
  it("holds no stream while the page is hidden, and opens one when it is shown", () => {
    const { sources, open, pending, later } = harness()
    let hidden = true
    let change = () => {}
    let watching = true
    const visibility = {
      hidden: () => hidden,
      onChange: (run: () => void) => {
        change = run
        return () => {
          watching = false
        }
      },
    }
    const stop = follow(open, "/e", { event: () => {} }, { later, visibility })
    expect(sources.length, "opened hidden: nothing").toBe(0)
    hidden = false
    change()
    expect(sources.length).toBe(1)
    change()
    expect(sources.length, "shown twice: still one").toBe(1)
    hidden = true
    change()
    expect(sources[0]?.closed, "hidden: closed").toBe(true)
    // One the browser gave up on is not opened again while hidden.
    hidden = false
    change()
    const second = sources[1]
    if (second) second.readyState = 2
    second?.emit("error")
    hidden = true
    pending.shift()?.()
    expect(sources.length).toBe(2)
    stop()
    expect(watching, "stopped watching the page").toBe(false)
  })
})
