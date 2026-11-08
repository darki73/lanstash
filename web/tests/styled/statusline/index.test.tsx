import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { StatusLine } from "@/styled/statusline"

describe("styled/statusline", () => {
  it("says what goes on, then its facts kept apart, and the action", () => {
    const { container } = render(
      <StatusLine
        mood="busy"
        words="Serving 2 clients"
        facts={["one", "two"]}
        action={<button type="button">Day</button>}
      />,
    )
    expect(screen.getByRole("region", { name: "Status" })).toBeTruthy()
    expect(screen.getByText("Serving 2 clients")).toBeTruthy()
    expect(screen.getByText("one")).toBeTruthy()
    expect(screen.getAllByText("·")).toHaveLength(1)
    expect(screen.getByRole("button", { name: "Day" })).toBeTruthy()
    expect(container.querySelector(".animate-beat")).toBeTruthy()
  })
  it("draws trouble's dot red, and idle's faint and still", () => {
    const { container, rerender } = render(<StatusLine mood="trouble" words="x" />)
    expect(container.querySelector(".bg-danger")).toBeTruthy()
    rerender(<StatusLine mood="idle" words="x" />)
    expect(container.querySelector(".animate-beat")).toBeNull()
  })
})
