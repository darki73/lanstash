/**
 * Hidden: the stash's entries with the games kept private marked, before any page draws them. An
 * entry is a hidden game's when its game (a Steam depot's app, else the entry itself) is on the
 * list; it is masked (its name replaced, so every page that names it names it so) or, for a
 * browser that shows hidden games, revealed (marked only). An entry lanstash masked itself (for
 * someone who is not an admin, `hidden`) is masked whatever the list.
 */
import { HIDDEN_NAME, type Hidden, hiddenKey } from "@/contracts/hidden"
import type { UsageRow } from "@/contracts/status"

export function markHidden(usage: readonly UsageRow[], hidden: Hidden | null | undefined, reveal: boolean): UsageRow[] {
  const keys = new Set(hidden ?? [])
  return usage.map((u) => {
    // lanstash masked it already (shown to someone who is not an admin).
    if (u.hidden === true) return { ...u, name: HIDDEN_NAME, privacy: "masked" }
    const mine = keys.has(hiddenKey(u.service, u.game)) || (u.group ? keys.has(hiddenKey(u.service, u.group)) : false)
    if (!mine) return u
    const marked: UsageRow = reveal ? { ...u, privacy: "revealed" } : { ...u, name: HIDDEN_NAME, privacy: "masked" }
    return marked
  })
}
