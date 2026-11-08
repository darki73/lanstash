import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Signal } from "@/styled/signal"

describe("styled/signal", () => {
  it("says what the colour means", () => {
    render(<Signal tone="good">live</Signal>)
    expect(screen.getByText("live")).toBeTruthy()
  })
  it("beats when good, unless still", () => {
    const { container } = render(
      <>
        <Signal tone="good">a</Signal>
        <Signal tone="good" still>
          b
        </Signal>
      </>,
    )
    expect(container.querySelectorAll(".animate-beat")).toHaveLength(1)
  })
  it("says more on hover when given a title", () => {
    render(
      <>
        <Signal tone="waiting" title="4 GB of 10 GB stashed">
          40 %
        </Signal>
        <Signal tone="bad">down</Signal>
      </>,
    )
    expect(screen.getByText("40 %").closest("[title]")?.getAttribute("title")).toBe("4 GB of 10 GB stashed")
    expect(screen.getByText("down").closest("[title]")?.getAttribute("title")).toBe("down")
  })
})
