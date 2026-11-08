/**
 * Verifies:
 *   - every hand-written source file starts with a doc comment saying what it is for;
 *   - no comment in src/ or the mirrored tests talks about history: comments describe the code
 *     as it is.
 */
import { describe, expect, it } from "vitest"
import { handWritten, isCode, read, src, tests, treeWalk, walk } from "./files"

const history = new RegExp(
  ["used to be", "we used to", "it used to", "the old way", "no longer", "formerly"].join("|"),
  "i",
)

describe("architecture: comments", { timeout: treeWalk }, () => {
  it("starts every hand-written source file with a doc comment", () => {
    const bare = handWritten().filter((p) => !read(src, p).startsWith("/**"))
    expect(bare).toEqual([])
  })
  it("never talks about history", () => {
    const sources = handWritten().map((p) => ({ root: src, p }))
    const mirrored = walk(tests)
      .filter((p) => isCode(p) && !p.startsWith("architecture/"))
      .map((p) => ({ root: tests, p }))
    const found = [...sources, ...mirrored].filter(({ root, p }) => history.test(read(root, p))).map(({ p }) => p)
    expect(found).toEqual([])
  })
})
