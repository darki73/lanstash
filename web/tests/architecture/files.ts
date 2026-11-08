/**
 * Files: walks the web tree for the guardrails. Paths are relative to the root walked, with
 * forward slashes on every system. Each tree is walked and each file read once per test file,
 * and a test that goes over the whole tree takes the treeWalk timeout, since its work grows with
 * the tree and slows when the machine is busy.
 */
import { readdirSync, readFileSync } from "node:fs"
import { join, relative } from "node:path"

export const web = join(import.meta.dirname, "..", "..")
export const src = join(web, "src")
export const tests = join(web, "tests")

/** How long a test going over the whole tree may take: its work grows with the tree. */
export const treeWalk = 60_000

const skipped = new Set(["node_modules", "dist", ".git"])

const walked = new Map<string, readonly string[]>()
const contents = new Map<string, string>()

/** walk lists every file under root, sorted; a root is walked once and the list kept. */
export function walk(root: string): string[] {
  let found = walked.get(root)
  if (found === undefined) {
    found = walkOnce(root)
    walked.set(root, found)
  }
  return [...found]
}

function walkOnce(root: string): string[] {
  const found: string[] = []
  const visit = (dir: string) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (skipped.has(entry.name)) continue
      const full = join(dir, entry.name)
      if (entry.isDirectory()) visit(full)
      else found.push(relative(root, full).split("\\").join("/"))
    }
  }
  visit(root)
  return found.sort()
}

/** read is a file's text; a file is read once and its text kept. */
export function read(root: string, path: string): string {
  const full = join(root, path)
  let text = contents.get(full)
  if (text === undefined) {
    text = readFileSync(full, "utf8")
    contents.set(full, text)
  }
  return text
}

/** TypeScript source: .ts or .tsx. */
export const isCode = (path: string) => /\.tsx?$/.test(path)

export const folderOf = (path: string) => (path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "")

/** Every source file under src/ (all of it is written by hand). */
export const handWritten = () => walk(src).filter(isCode)
