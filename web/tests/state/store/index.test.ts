import { describe, expect, it, vi } from "vitest"
import { createStore } from "@/state/store"

describe("state/store", () => {
  it("holds a value and tells subscribers when it changes", () => {
    const store = createStore(1)
    const listener = vi.fn()
    store.subscribe(listener)
    store.set((n) => n + 1)
    expect(store.get()).toBe(2)
    expect(listener).toHaveBeenCalledTimes(1)
  })
  it("stays quiet when set to the same value", () => {
    const store = createStore("a")
    const listener = vi.fn()
    store.subscribe(listener)
    store.set("a")
    expect(listener).not.toHaveBeenCalled()
  })
  it("stops telling a subscriber that left", () => {
    const store = createStore(0)
    const listener = vi.fn()
    const leave = store.subscribe(listener)
    leave()
    store.set(1)
    expect(listener).not.toHaveBeenCalled()
  })
})
