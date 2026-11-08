import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Fold } from "@/styled/fold"

describe("styled/fold", () => {
  it("starts closed, with its title and summary on the line that opens it", () => {
    const { container } = render(
      <Fold title="Launcher data" summary="319 KB">
        inside
      </Fold>,
    )
    const details = container.querySelector("details")
    expect(details?.open).toBe(false)
    expect(container.querySelector("summary")?.textContent).toContain("Launcher data")
    expect(screen.getByText("319 KB")).toBeTruthy()
  })
})
