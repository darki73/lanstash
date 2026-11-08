import { act, render } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { type DarkQuery, resolveTheme, ThemeProvider } from "@/styled/theme"

function query(matches: boolean) {
  const listeners = new Set<() => void>()
  const q = {
    matches,
    addEventListener: (_: "change", l: () => void) => listeners.add(l),
    removeEventListener: (_: "change", l: () => void) => listeners.delete(l),
    flip() {
      q.matches = !q.matches
      for (const l of listeners) l()
    },
  }
  return q satisfies DarkQuery
}

describe("styled/theme", () => {
  it("resolves a mode", () => {
    expect(resolveTheme("system", true)).toBe("dark")
    expect(resolveTheme("system", false)).toBe("light")
    expect(resolveTheme("light", true)).toBe("light")
  })
  it("sets data-theme and follows the system in system mode", () => {
    const element = document.createElement("div")
    const q = query(false)
    render(<ThemeProvider mode="system" query={q} element={element} />)
    expect(element.dataset.theme).toBe("light")
    act(() => q.flip())
    expect(element.dataset.theme).toBe("dark")
  })
})
