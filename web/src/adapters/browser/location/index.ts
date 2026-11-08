/**
 * Location: the app's path, the path of the page's address ("/platforms/blizzard/osi"), and its
 * changes. Going somewhere pushes the path onto the browser's history; going back or forward
 * tells whoever subscribed. A plain click on a link to a path of the page's own (no modifier key,
 * no target, not a download) goes there without loading the page again; any other click is the
 * browser's. An address with the path in its hash ("#/overview/storage"), is moved
 * to the path. The server must answer the page for any path that is not the API's. The window is
 * passed in: the browser's in the app, a fake in tests.
 */

/** A clicked element, as far as finding its link goes. */
export interface ClickTarget {
  closest(selector: "a"): { getAttribute(name: string): string | null } | null
}

/** A click, as far as this adapter reads it. */
export interface Click {
  readonly defaultPrevented: boolean
  readonly button: number
  readonly metaKey: boolean
  readonly ctrlKey: boolean
  readonly shiftKey: boolean
  readonly altKey: boolean
  readonly target: unknown
  preventDefault(): void
}

/** The part of the browser's window this adapter uses. */
export interface PathWindow {
  readonly location: { readonly pathname: string; readonly hash: string }
  readonly history: {
    pushState(data: null, unused: string, url: string): void
    replaceState(data: null, unused: string, url: string): void
  }
  addEventListener(type: "popstate", listener: () => void): void
  addEventListener(type: "click", listener: (e: Click) => void): void
  removeEventListener(type: "popstate", listener: () => void): void
  removeEventListener(type: "click", listener: (e: Click) => void): void
}

export interface Location {
  get(): string
  subscribe(listener: () => void): () => void
  go(path: string): void
}

const isTarget = (t: unknown): t is ClickTarget =>
  typeof t === "object" && t !== null && typeof (t as { closest?: unknown }).closest === "function"

/** The app path a click on a link goes to, if it is one this page should follow itself. */
function followed(e: Click): string | undefined {
  if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return undefined
  const a = isTarget(e.target) ? e.target.closest("a") : null
  const href = a?.getAttribute("href")
  if (a == null || href == null || a.getAttribute("target") || a.getAttribute("download") !== null) return undefined
  // Only paths of the page's own ("/x"), not other origins ("//host", "http:") nor the API.
  if (!href.startsWith("/") || href.startsWith("//") || href.startsWith("/api/")) return undefined
  return href
}

export function pathLocation(w: PathWindow): Location {
  // An old `#/x` link becomes the path `/x`; `#//host` or `#/\host` would name another origin,
  // which the browser refuses (throwing): such a hash is left as it is.
  const hash = w.location.hash.slice(1)
  if (/^\/(?![/\\])/.test(hash)) {
    try {
      w.history.replaceState(null, "", hash)
    } catch {
      // Not a path of this page after all: the page starts where it is.
    }
  }
  const listeners = new Set<() => void>()
  const tell = () => {
    for (const l of [...listeners]) l()
  }
  const get = () => w.location.pathname || "/"
  const go = (path: string) => {
    if (path === get()) return
    w.history.pushState(null, "", path)
    tell()
  }
  const click = (e: Click) => {
    const path = followed(e)
    if (path === undefined) return
    e.preventDefault()
    go(path)
  }
  return {
    get,
    go,
    subscribe(listener) {
      if (listeners.size === 0) {
        w.addEventListener("popstate", tell)
        w.addEventListener("click", click)
      }
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
        if (listeners.size === 0) {
          w.removeEventListener("popstate", tell)
          w.removeEventListener("click", click)
        }
      }
    },
  }
}
