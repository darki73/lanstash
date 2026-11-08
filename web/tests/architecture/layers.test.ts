/**
 * Verifies:
 *   - imports go only one way: base, then styled, then the features, then the composition root
 *     (app) and the entry; features never reach base directly;
 *   - the non-UI layers (contracts, state, adapters, lib) import only what the table allows:
 *     state never sees an adapter, adapters never see state; only app joins them;
 *   - every top-level folder of src/ is a known layer;
 *   - React is never imported by the non-UI layers, except state's useStore hook;
 *   - simple-icons (the platforms' marks) is imported by styled only, i18next and react-i18next
 *     by i18n only.
 */
import { describe, expect, it } from "vitest"
import { handWritten, read, src, treeWalk } from "./files"
import { internalTarget, layerOf, specifiers } from "./imports"

const allowed: Readonly<Record<string, readonly string[]>> = {
  lib: ["lib"],
  contracts: ["contracts"],
  adapters: ["adapters", "lib"],
  state: ["state", "contracts", "lib"],
  i18n: ["i18n", "lib"],
  base: ["base", "lib"],
  styled: ["styled", "base", "i18n", "lib"],
  features: ["features", "styled", "contracts", "state", "i18n", "lib"],
  app: ["app", "features", "styled", "contracts", "state", "adapters", "i18n", "lib"],
  entry: ["entry", "app"],
}

/**
 * Packages only one layer may import: the platforms' marks are drawn by styled alone; the words
 * come through i18n alone.
 */
const packageOwners: Readonly<Record<string, string>> = {
  "simple-icons": "styled",
  i18next: "i18n",
  "react-i18next": "i18n",
}

/** The layers that hold no components. */
const plain = ["lib", "contracts", "adapters", "state"]

/** The one file of those that may use React: the hook components read stores with. */
const reactAllowed = ["state/store/use.ts"]

const imports = () =>
  handWritten().flatMap((file) => specifiers(read(src, file)).map((specifier) => ({ file, specifier })))

describe("architecture: layers", { timeout: treeWalk }, () => {
  it("knows every top-level folder of src", () => {
    const unknown = [...new Set(handWritten().map(layerOf))].filter((layer) => !(layer in allowed))
    expect(unknown).toEqual([])
  })
  it("imports only from the layers the table allows", () => {
    const wrong = imports()
      .map(({ file, specifier }) => ({ file, target: internalTarget(file, specifier) }))
      .filter(({ file, target }) => target !== undefined && !allowed[layerOf(file)]?.includes(layerOf(target)))
      .map(({ file, target }) => `${file} -> ${target}`)
    expect(wrong).toEqual([])
  })
  it("keeps a package to the layer that owns it", () => {
    const wrong = imports()
      .filter(({ file, specifier }) =>
        Object.entries(packageOwners).some(
          ([pkg, owner]) => (specifier === pkg || specifier.startsWith(`${pkg}/`)) && layerOf(file) !== owner,
        ),
      )
      .map(({ file, specifier }) => `${file} -> ${specifier}`)
    expect(wrong).toEqual([])
  })
  it("keeps React out of the layers without components", () => {
    const wrong = imports()
      .filter(({ file }) => plain.includes(layerOf(file)) && !reactAllowed.includes(file))
      .filter(
        ({ specifier }) => specifier === "react" || specifier.startsWith("react-dom") || specifier.startsWith("react/"),
      )
      .map(({ file, specifier }) => `${file} -> ${specifier}`)
    expect(wrong).toEqual([])
  })
})
