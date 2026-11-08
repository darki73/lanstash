import { fireEvent, render } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Monogram } from "@/styled/monogram"

describe("styled/monogram", () => {
  it("draws its letters, hidden from assistive technology", () => {
    const { container } = render(<Monogram letters="WS" />)
    expect(container.textContent).toBe("WS")
    expect(container.firstElementChild?.getAttribute("aria-hidden")).toBe("true")
  })
  it("draws its icon instead when it has one, and its letters again when the icon fails", () => {
    const { container, rerender } = render(<Monogram letters="St" image="/art/sc2/icon" />)
    const img = container.querySelector("img")
    expect(img?.getAttribute("src")).toBe("/art/sc2/icon")
    expect(container.textContent).toBe("")
    expect(container.firstElementChild?.getAttribute("aria-hidden")).toBe("true")
    if (img) fireEvent.error(img)
    expect(container.querySelector("img")).toBeNull()
    expect(container.textContent).toBe("St")
    rerender(<Monogram letters="He" image="/art/hero/icon" />)
    expect(container.querySelector("img")?.getAttribute("src")).toBe("/art/hero/icon")
  })
})
