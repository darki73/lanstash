/**
 * Pages: the frame's pages, each loaded when first shown (React.lazy), so the first load carries
 * the frame and the page asked for, not every page there is.
 */
import { lazy } from "react"

export const Overview = lazy(() => import("@/features/status").then((m) => ({ default: m.Overview })))
export const SettingsView = lazy(() => import("@/features/settings").then((m) => ({ default: m.SettingsView })))
export const DnsView = lazy(() => import("@/features/dns").then((m) => ({ default: m.DnsView })))
export const HistoryView = lazy(() => import("@/features/history").then((m) => ({ default: m.HistoryView })))
export const HealthView = lazy(() => import("@/features/health").then((m) => ({ default: m.HealthView })))
export const ClientsView = lazy(() => import("@/features/clients").then((m) => ({ default: m.ClientsView })))
export const UsersView = lazy(() => import("@/features/access/users").then((m) => ({ default: m.UsersView })))
export const OwnAccount = lazy(() => import("@/features/access/own").then((m) => ({ default: m.OwnAccount })))
export const Join = lazy(() => import("@/features/access/join").then((m) => ({ default: m.Join })))
export const PlatformView = lazy(() => import("@/features/platforms").then((m) => ({ default: m.PlatformView })))
export const GameView = lazy(() => import("@/features/platforms/game").then((m) => ({ default: m.GameView })))
