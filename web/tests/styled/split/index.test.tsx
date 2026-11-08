import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Split } from "@/styled/split"

describe("styled/split", () => {
  it("puts the main parts first and the side ones after them", () => {
    const { container } = render(<Split side={<p>side</p>}>main</Split>)
    expect(container.textContent).toBe("mainside")
    expect(screen.getByText("side")).toBeTruthy()
  })
})
