import { describe, expect, it } from "vitest"
import { type Click, type PathWindow, pathLocation } from "@/adapters/browser/location"

/** A window with a history: paths pushed, the back button, and clicks. */
function fakeWindow(path: string, hash = "") {
  const pop = new Set<() => void>()
  const clicks = new Set<(e: Click) => void>()
  const stack = [path]
  const w = {
    location: { pathname: path, hash },
    history: {
      pushState(_: null, __: string, url: string) {
        stack.push(url)
        w.location.pathname = url
      },
      replaceState(_: null, __: string, url: string) {
        stack[stack.length - 1] = url
        w.location.pathname = url
        w.location.hash = ""
      },
    },
    addEventListener(type: "popstate" | "click", l: (() => void) | ((e: Click) => void)) {
      if (type === "popstate") pop.add(l as () => void)
      else clicks.add(l as (e: Click) => void)
    },
    removeEventListener(type: "popstate" | "click", l: (() => void) | ((e: Click) => void)) {
      if (type === "popstate") pop.delete(l as () => void)
      else clicks.delete(l as (e: Click) => void)
    },
    back() {
      stack.pop()
      w.location.pathname = stack[stack.length - 1] ?? "/"
      for (const l of pop) l()
    },
    /** Clicks a link with these attributes; true when the page followed it itself. */
    click(attrs: Record<string, string>, keys: Partial<Click> = {}) {
      let prevented = false
      const e: Click = {
        defaultPrevented: false,
        button: 0,
        metaKey: false,
        ctrlKey: false,
        shiftKey: false,
        altKey: false,
        target: { closest: () => ({ getAttribute: (n: string) => attrs[n] ?? null }) },
        preventDefault: () => {
          prevented = true
        },
        ...keys,
      }
      for (const l of clicks) l(e)
      return prevented
    },
    listening: () => pop.size + clicks.size,
    stack,
  }
  return w satisfies PathWindow
}

describe("adapters/browser/location", () => {
  it("reads the path of the address, / when there is none", () => {
    expect(pathLocation(fakeWindow("/overview/storage")).get()).toBe("/overview/storage")
    expect(pathLocation(fakeWindow("")).get()).toBe("/")
  })
  it("moves an address with the path in its hash to the path", () => {
    const w = fakeWindow("/", "#/platforms/blizzard")
    expect(pathLocation(w).get()).toBe("/platforms/blizzard")
    expect(w.stack).toEqual(["/platforms/blizzard"])
  })
  it("leaves a hash that would lead off the page as it is, and never throws", () => {
    for (const hash of ["#//elsewhere.example", "#/\\elsewhere.example"]) {
      const w = fakeWindow("/", hash)
      expect(pathLocation(w).get(), hash).toBe("/")
      expect(w.stack).toEqual(["/"])
    }
    // A browser that refuses the move anyway (another origin): the page still starts.
    const w = fakeWindow("/", "#/x")
    w.history.replaceState = () => {
      throw new Error("SecurityError")
    }
    expect(pathLocation(w).get()).toBe("/")
  })
  it("pushes where it goes and tells subscribers, also of going back, until they leave", () => {
    const w = fakeWindow("/")
    const location = pathLocation(w)
    let heard = 0
    const leave = location.subscribe(() => heard++)
    location.go("/platforms/steam")
    location.go("/platforms/steam")
    expect([location.get(), heard, w.stack]).toEqual(["/platforms/steam", 1, ["/", "/platforms/steam"]])
    w.back()
    expect([location.get(), heard]).toEqual(["/", 2])
    leave()
    expect(w.listening()).toBe(0)
  })
  it("follows plain clicks on its own links, and leaves every other click to the browser", () => {
    const w = fakeWindow("/")
    const location = pathLocation(w)
    location.subscribe(() => {})
    expect(w.click({ href: "/platforms/blizzard/osi" })).toBe(true)
    expect(location.get()).toBe("/platforms/blizzard/osi")
    expect(w.click({ href: "/platforms/steam" }, { ctrlKey: true })).toBe(false)
    expect(w.click({ href: "/platforms/steam" }, { button: 1 })).toBe(false)
    expect(w.click({ href: "/platforms/steam", target: "_blank" })).toBe(false)
    expect(w.click({ href: "/api/v1/status" })).toBe(false)
    expect(w.click({ href: "https://blizzard.com/" })).toBe(false)
    expect(w.click({ href: "//elsewhere/x" })).toBe(false)
    expect(location.get()).toBe("/platforms/blizzard/osi")
  })
})
