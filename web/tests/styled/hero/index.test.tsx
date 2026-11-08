import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Hero } from "@/styled/hero"

describe("styled/hero", () => {
  it("fades the art in once loaded, crops a poster to its lower part, and shows its badge", () => {
    const { container } = render(<Hero image="/art/osi" cover="poster" badge={<span>current</span>} />)
    const img = container.querySelector("img")
    expect(img?.className).toContain("opacity-0")
    expect(img?.className).toContain("object-[center_80%]")
    if (img) fireEvent.load(img)
    expect(container.querySelector("img")?.className).toContain("opacity-100")
    expect(screen.getByText("current")).toBeTruthy()
  })
  it("leaves nothing when the art fails, and tries a new address", () => {
    const { container, rerender } = render(<Hero image="/art/x" />)
    const img = container.querySelector("img")
    if (img) fireEvent.error(img)
    expect(container.querySelector("img")).toBeNull()
    rerender(<Hero image="/art/y" />)
    expect(container.querySelector("img")?.getAttribute("src")).toBe("/art/y")
  })
})
