/**
 * Verifies:
 *   - a folder holds .tsx components or .ts logic, never both (in src/ and in tests/);
 *   - every src/ folder holding hand-written source has an index.tsx or index.ts.
 */
import { describe, expect, it } from "vitest"
import { folderOf, handWritten, isCode, src, tests, treeWalk, walk } from "./files"

function mixedFolders(paths: readonly string[]): string[] {
  const kinds = new Map<string, Set<string>>()
  for (const path of paths.filter(isCode)) {
    const folder = folderOf(path)
    const set = kinds.get(folder) ?? new Set<string>()
    set.add(path.endsWith(".tsx") ? "tsx" : "ts")
    kinds.set(folder, set)
  }
  return [...kinds].filter(([, set]) => set.size > 1).map(([folder]) => folder)
}

describe("architecture: folders", { timeout: treeWalk }, () => {
  it("never mixes .ts and .tsx in a src folder", () => {
    expect(mixedFolders(walk(src))).toEqual([])
  })
  it("never mixes .ts and .tsx in a tests folder", () => {
    expect(mixedFolders(walk(tests))).toEqual([])
  })
  it("gives every src folder an index", () => {
    const files = new Set(handWritten())
    const folders = new Set([...files].map(folderOf))
    const missing = [...folders].filter((f) => {
      const prefix = f === "" ? "" : `${f}/`
      return !files.has(`${prefix}index.ts`) && !files.has(`${prefix}index.tsx`)
    })
    expect(missing).toEqual([])
  })
})
