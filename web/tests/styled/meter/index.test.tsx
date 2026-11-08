import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Meter } from "@/styled/meter"

describe("styled/meter", () => {
  it("says the percentage, clamped", () => {
    render(<Meter fraction={1.7} />)
    expect(screen.getByRole("meter").getAttribute("value")).toBe("100")
    expect(screen.getByText("100 %")).toBeTruthy()
  })
  it("says a label when given one", () => {
    render(<Meter fraction={0} label="12.0 MB" />)
    expect(screen.getByText("12.0 MB")).toBeTruthy()
  })
})
