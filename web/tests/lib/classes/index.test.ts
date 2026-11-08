import { describe, expect, it } from "vitest"
import { classes } from "@/lib/classes"

describe("lib/classes", () => {
  it("joins the names that are set", () => {
    expect(classes("a", false, null, undefined, "", "b")).toBe("a b")
  })
})
