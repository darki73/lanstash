import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Toggles } from "@/styled/toggles"

describe("styled/toggles", () => {
  it("switches each pill on and off, several at once", () => {
    const onChange = vi.fn()
    const options = [
      { id: "steam/a", group: "steam", label: "a" },
      { id: "steam/b", group: "steam", label: "b" },
    ]
    render(<Toggles label="Links" options={options} values={["steam/a"]} onChange={onChange} />)
    const a = screen.getByRole("button", { name: "steam a" })
    expect(a.getAttribute("aria-pressed")).toBe("true")
    fireEvent.click(screen.getByRole("button", { name: "steam b" }))
    expect(onChange).toHaveBeenLastCalledWith(["steam/a", "steam/b"])
    fireEvent.click(a)
    expect(onChange).toHaveBeenLastCalledWith([])
  })
  it("says why there is nothing to choose", () => {
    render(<Toggles label="Links" options={[]} values={[]} onChange={() => {}} empty="none yet" />)
    expect(screen.getByText("none yet")).toBeTruthy()
  })
})
