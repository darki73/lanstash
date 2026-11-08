/**
 * Theme: provides the light, dark or system mode and sets data-theme on the document element to
 * the resolved one; in system mode it follows the system preference as it changes. The colour
 * tokens are in tokens.css. The media query and the element are passed in for tests (the
 * browser's prefers-color-scheme query and the document element by default).
 */
import { createContext, type ReactNode, useContext, useLayoutEffect, useSyncExternalStore } from "react"

export type ThemeMode = "light" | "dark" | "system"
export type ResolvedTheme = "light" | "dark"

/** The part of a MediaQueryList the provider uses. */
export interface DarkQuery {
  readonly matches: boolean
  addEventListener(type: "change", listener: () => void): void
  removeEventListener(type: "change", listener: () => void): void
}

export interface ThemeValue {
  readonly mode: ThemeMode
  readonly resolved: ResolvedTheme
}

const Theme = createContext<ThemeValue>({ mode: "system", resolved: "light" })

/** useTheme is the mode in force and what it resolved to. */
export function useTheme(): ThemeValue {
  return useContext(Theme)
}

/** resolveTheme is the theme a mode draws in, given whether the system prefers dark. */
export function resolveTheme(mode: ThemeMode, systemDark: boolean): ResolvedTheme {
  if (mode === "system") return systemDark ? "dark" : "light"
  return mode
}

const browserQuery = (): DarkQuery | undefined =>
  typeof window === "undefined" || !window.matchMedia ? undefined : window.matchMedia("(prefers-color-scheme: dark)")

export interface ThemeProviderProps {
  readonly mode: ThemeMode
  readonly query?: DarkQuery
  readonly element?: HTMLElement
  readonly children?: ReactNode
}

export function ThemeProvider({ mode, query = browserQuery(), element, children }: ThemeProviderProps) {
  const systemDark = useSyncExternalStore(
    (changed) => {
      query?.addEventListener("change", changed)
      return () => query?.removeEventListener("change", changed)
    },
    () => query?.matches ?? false,
    () => false,
  )
  const resolved = resolveTheme(mode, systemDark)
  useLayoutEffect(() => {
    const target = element ?? document.documentElement
    target.dataset.theme = resolved
  }, [element, resolved])
  return <Theme.Provider value={{ mode, resolved }}>{children}</Theme.Provider>
}
