import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Toggle } from "@/styled/toggle"

describe("styled/toggle", () => {
  it("says whether it is open and calls back when pressed", () => {
    const onToggle = vi.fn()
    render(
      <Toggle open={false} onToggle={onToggle}>
        3 pieces
      </Toggle>,
    )
    const button = screen.getByRole("button")
    expect([button.textContent, button.getAttribute("aria-expanded")]).toEqual(["▸ 3 pieces", "false"])
    fireEvent.click(button)
    expect(onToggle).toHaveBeenCalledTimes(1)
  })
})
