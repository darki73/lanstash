/**
 * Verifies:
 *   - every test under tests/ (outside tests/architecture) mirrors a source file:
 *     tests/x/y/index.test.tsx needs src/x/y/index.tsx, with the same extension;
 *   - every hand-written source file has its mirrored test;
 *   - tests/ holds nothing but tests outside tests/architecture and tests/kit (shared fakes).
 */
import { existsSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, it } from "vitest"
import { handWritten, src, tests, treeWalk, walk } from "./files"

const testFiles = walk(tests).filter((p) => !p.startsWith("architecture/") && !p.startsWith("kit/"))
const sourceOf = (test: string) => test.replace(/\.test\.(tsx?)$/, ".$1")
const testOf = (source: string) => source.replace(/\.(tsx?)$/, ".test.$1")

describe("architecture: mirrored tests", { timeout: treeWalk }, () => {
  it("holds only tests outside tests/architecture and tests/kit", () => {
    expect(testFiles.filter((p) => !/\.test\.tsx?$/.test(p))).toEqual([])
  })
  it("mirrors a source file with every test", () => {
    const orphans = testFiles.filter((p) => /\.test\.tsx?$/.test(p) && !existsSync(join(src, sourceOf(p))))
    expect(orphans).toEqual([])
  })
  it("gives every hand-written source file a test", () => {
    const untested = handWritten().filter((p) => !existsSync(join(tests, testOf(p))))
    expect(untested).toEqual([])
  })
})
