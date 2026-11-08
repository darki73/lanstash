import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Segmented } from "@/styled/segmented"

describe("styled/segmented", () => {
  it("presses the chosen option and reports a new choice", () => {
    const onChange = vi.fn()
    render(
      <Segmented
        label="Sort the games"
        options={[
          { id: "size", label: "Size" },
          { id: "name", label: "Name" },
        ]}
        value="size"
        onChange={onChange}
      />,
    )
    expect(screen.getByRole("group", { name: "Sort the games" })).toBeTruthy()
    expect(screen.getByRole("button", { name: "Size" }).getAttribute("aria-pressed")).toBe("true")
    fireEvent.click(screen.getByRole("button", { name: "Name" }))
    expect(onChange).toHaveBeenCalledWith("name")
  })
})
