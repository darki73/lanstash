import { render } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Glyph } from "@/styled/glyph"

describe("styled/glyph", () => {
  it("draws its icon, hidden from assistive technology", () => {
    const { container } = render(<Glyph name="overview" />)
    expect(container.firstElementChild?.getAttribute("aria-hidden")).toBe("true")
    expect(container.querySelector("path")?.getAttribute("d")).toMatch(/^M3 13/)
  })
})
