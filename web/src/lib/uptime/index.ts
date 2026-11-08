/**
 * Uptime: how long something has run, as whole hours (growing without a day unit) and the minutes
 * and seconds past them, each two digits (the words around them are the reader's language's).
 */
export function uptime(ms: number): { readonly h: number; readonly mm: string; readonly ss: string } {
  const t = Math.max(0, Math.floor(ms / 1000))
  return {
    h: Math.floor(t / 3600),
    mm: String(Math.floor(t / 60) % 60).padStart(2, "0"),
    ss: String(t % 60).padStart(2, "0"),
  }
}
