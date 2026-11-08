/**
 * Verifies:
 *   - a feature file draws only with styled pieces: it never sets className and
 *     never writes a raw HTML element, so how a card, a list or a status line looks is decided in
 *     one file of the styled layer;
 *   - the files that still do are listed in styling.allow; the list may only shrink: a file not
 *     on it never may, and a listed file that does not (or is gone) must leave the list.
 */
import { readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, it } from "vitest"
import { handWritten, read, src, treeWalk } from "./files"

/** The HTML elements a feature reaches through styled pieces instead. */
const raw =
  /<(div|span|p|ul|ol|li|h[1-6]|section|header|footer|nav|main|article|aside|dl|dt|dd|table|thead|tbody|tr|td|th|button|a|input|select|textarea|label|code|pre|strong|em|small|kbd|form|fieldset|legend|details|summary|img|svg)[\s>/]/

/** stylesItself says whether a source sets classes or draws raw HTML itself. */
function stylesItself(source: string): boolean {
  return /\bclassName\s*=/.test(source) || raw.test(source)
}

const allowed = () =>
  readFileSync(join(import.meta.dirname, "styling.allow"), "utf8")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "" && !line.startsWith("#"))

/** The files that compose pages: the features. */
const features = () => handWritten().filter((p) => p.startsWith("features/") && p.endsWith(".tsx"))

describe("architecture: styling", { timeout: treeWalk }, () => {
  it("knows raw HTML and className when it sees them", () => {
    expect(stylesItself(`<div className="x">`)).toBe(true)
    expect(stylesItself("<p>words</p>")).toBe(true)
    expect(stylesItself("<Stack gap='snug'><Note>words</Note></Stack>")).toBe(false)
    expect(stylesItself("<Panel title='x' />")).toBe(false)
  })
  it("covers the features", () => {
    expect(features().length).toBeGreaterThan(0)
  })
  it("lets no feature file style itself unless it is on the shrinking list", () => {
    const listed = new Set(allowed())
    const wrong = features().filter((file) => stylesItself(read(src, file)) && !listed.has(file))
    expect(wrong).toEqual([])
  })
  it("keeps the list to files that still style themselves", () => {
    const styling = new Set(features().filter((file) => stylesItself(read(src, file))))
    expect(allowed().filter((file) => !styling.has(file))).toEqual([])
  })
})
