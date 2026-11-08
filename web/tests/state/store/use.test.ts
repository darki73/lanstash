import { act, renderHook } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { createStore } from "@/state/store"
import { useStore } from "@/state/store/use"

describe("state/store/use", () => {
  it("reads a store and follows its changes", () => {
    const store = createStore(1)
    const { result } = renderHook(() => useStore(store))
    expect(result.current).toBe(1)
    act(() => store.set(2))
    expect(result.current).toBe(2)
  })
})
