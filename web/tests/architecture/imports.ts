/**
 * Imports: reads the import specifiers of a source file and resolves the ones inside src/ to
 * src-relative paths. "@/x" is src/x; "./x" and "../x" are relative to the importing file.
 */
import { posix } from "node:path"
import { folderOf } from "./files"

const pattern =
  /(?:^|[\s;])(?:import|export)\s[^"';]*?from\s*["']([^"']+)["']|import\s*\(\s*["']([^"']+)["']\s*\)|^\s*import\s+["']([^"']+)["']/gm

export function specifiers(text: string): string[] {
  return [...text.matchAll(pattern)].map((m) => m[1] ?? m[2] ?? m[3] ?? "").filter((s) => s !== "")
}

/** The src-relative path an import points at, or undefined for a package. */
export function internalTarget(from: string, specifier: string): string | undefined {
  if (specifier.startsWith("@/")) return specifier.slice(2)
  if (specifier.startsWith("./") || specifier.startsWith("../")) {
    return posix.normalize(posix.join(folderOf(from), specifier))
  }
  return undefined
}

/**
 * The layer a src-relative path belongs to: its first folder, or "entry" for files at the top.
 * An import of a folder ("@/root") names the folder's index, so it belongs to that folder.
 */
export function layerOf(path: string): string {
  if (path.includes("/")) return path.slice(0, path.indexOf("/"))
  return path.includes(".") ? "entry" : path
}
