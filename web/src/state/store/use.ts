/**
 * useStore: reads a store (or anything that can be read and subscribed to) in a component and
 * draws it again when it changes.
 */
import { useSyncExternalStore } from "react"
import type { Store } from "@/state/store"

export type Readable<T> = Pick<Store<T>, "get" | "subscribe">

export function useStore<T>(store: Readable<T>): T {
  return useSyncExternalStore(store.subscribe, store.get, store.get)
}
