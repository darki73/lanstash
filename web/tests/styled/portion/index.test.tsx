import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Portion } from "@/styled/portion"

describe("styled/portion", () => {
  it("draws the figure, a bar split into its parts, and each part's figure", () => {
    const { container } = render(
      <Portion
        value="4 GB"
        caption="got"
        parts={[
          { label: "from the stash", value: "3 GB", share: 0.75, tone: "cache" },
          { label: "from the internet", value: "1 GB", share: 0.25, tone: "origin" },
        ]}
      />,
    )
    expect(screen.getByText("4 GB")).toBeTruthy()
    expect(screen.getByText("from the stash").tagName).toBe("DT")
    const widths = [...container.querySelectorAll<HTMLElement>("[style]")].map((e) => e.style.width)
    expect(widths).toEqual(["75%", "25%"])
  })
})
