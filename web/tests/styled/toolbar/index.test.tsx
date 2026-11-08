import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Toolbar } from "@/styled/toolbar"

describe("styled/toolbar", () => {
  it("draws the title and the tools", () => {
    render(<Toolbar title="5 games in the cache" tools={<button type="button">Size</button>} />)
    expect(screen.getByRole("heading", { level: 3 }).textContent).toBe("5 games in the cache")
    expect(screen.getByRole("button", { name: "Size" })).toBeTruthy()
  })
})
