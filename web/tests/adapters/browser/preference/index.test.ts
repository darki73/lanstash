import { describe, expect, it } from "vitest"
import { choice, type PreferenceStorage, preference } from "@/adapters/browser/preference"

const memory = (): PreferenceStorage & { kept: Map<string, string> } => {
  const kept = new Map<string, string>()
  return {
    kept,
    getItem: (k) => kept.get(k) ?? null,
    setItem: (k, v) => void kept.set(k, v),
    removeItem: (k) => void kept.delete(k),
  }
}

describe("adapters/browser/preference", () => {
  it("remembers a choice, and forgets it when turned off", () => {
    const s = memory()
    const p = preference(s, "lanstash.x")
    expect(p.read()).toBe(false)
    p.write(true)
    expect([p.read(), s.kept.get("lanstash.x")]).toEqual([true, "yes"])
    p.write(false)
    expect([p.read(), s.kept.has("lanstash.x")]).toEqual([false, false])
  })
  it("reads no from storage that is missing or blocked, and never throws", () => {
    const blocked: PreferenceStorage = {
      getItem: () => {
        throw new Error("denied")
      },
      setItem: () => {
        throw new Error("denied")
      },
      removeItem: () => {
        throw new Error("denied")
      },
    }
    for (const s of [undefined, blocked]) {
      const p = preference(s, "k")
      expect(p.read()).toBe(false)
      expect(() => p.write(true)).not.toThrow()
      const c = choice(s, "k")
      expect(c.read()).toBeUndefined()
      expect(() => c.write("de")).not.toThrow()
    }
  })
  it("remembers a word, and forgets it", () => {
    const s = memory()
    const c = choice(s, "lanstash.language")
    expect(c.read()).toBeUndefined()
    c.write("de")
    expect(c.read()).toBe("de")
    c.write(undefined)
    expect(s.kept.has("lanstash.language")).toBe(false)
  })
})
