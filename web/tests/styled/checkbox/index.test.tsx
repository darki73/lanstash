import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Checkbox } from "@/styled/checkbox"

describe("styled/checkbox", () => {
  it("is ticked or not, partly when some is chosen, and reports a click", () => {
    const onChange = vi.fn()
    const { rerender } = render(<Checkbox checked={false} mixed label="Choose all" onChange={onChange} />)
    const box = screen.getByRole("checkbox", { name: "Choose all" }) as HTMLInputElement
    expect(box.indeterminate).toBe(true)
    fireEvent.click(box)
    expect(onChange).toHaveBeenCalledWith(true)
    rerender(<Checkbox checked label="Choose all" onChange={onChange} />)
    expect(box.checked && !box.indeterminate).toBe(true)
  })
})
